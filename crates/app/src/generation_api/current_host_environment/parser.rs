use std::collections::BTreeSet;
use std::io::Read;

use super::CurrentHostEnvironmentError;

pub(super) const MEMORY_ROUNDING_GRANULARITY_MIB: u64 = 1_024;
const MAX_CPU_INDEX: u32 = 8_191;
const MAX_CGROUP_DEPTH: usize = 32;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CpuInventory {
    pub(super) cpu_model: String,
    pub(super) logical_cpus: BTreeSet<u32>,
    pub(super) physical_core_count: u32,
}

pub(super) fn parse_kernel_release(bytes: &[u8]) -> Result<String, CurrentHostEnvironmentError> {
    let release = exact_one_line(bytes)?;
    if !release
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    Ok(release.to_owned())
}

pub(super) fn parse_online_cpu_list(
    bytes: &[u8],
) -> Result<BTreeSet<u32>, CurrentHostEnvironmentError> {
    parse_cpu_list(exact_one_line(bytes)?)
}

pub(super) fn validate_affinity(
    online_cpus: &BTreeSet<u32>,
    maximum_cpu_count: usize,
    mut is_set: impl FnMut(usize) -> bool,
) -> Result<(), CurrentHostEnvironmentError> {
    if maximum_cpu_count == 0
        || online_cpus
            .last()
            .is_none_or(|cpu| usize::try_from(*cpu).map_or(true, |cpu| cpu >= maximum_cpu_count))
    {
        return Err(CurrentHostEnvironmentError::ObservationUnavailable);
    }
    let exact = (0..maximum_cpu_count)
        .all(|cpu| is_set(cpu) == online_cpus.contains(&u32::try_from(cpu).unwrap_or(u32::MAX)));
    if exact {
        Ok(())
    } else {
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    }
}

pub(super) fn parse_cpu_list(text: &str) -> Result<BTreeSet<u32>, CurrentHostEnvironmentError> {
    if text.is_empty() || text.trim() != text {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    let mut cpus = BTreeSet::new();
    let mut prior_end = None;
    for segment in text.split(',') {
        let (start, end) = if let Some((start, end)) = segment.split_once('-') {
            if end.contains('-') {
                return Err(CurrentHostEnvironmentError::InvalidObservation);
            }
            (parse_cpu_index(start)?, parse_cpu_index(end)?)
        } else {
            let value = parse_cpu_index(segment)?;
            (value, value)
        };
        if start > end || prior_end.is_some_and(|prior| start <= prior) {
            return Err(CurrentHostEnvironmentError::InvalidObservation);
        }
        for cpu in start..=end {
            if !cpus.insert(cpu) {
                return Err(CurrentHostEnvironmentError::InvalidObservation);
            }
        }
        prior_end = Some(end);
    }
    if cpus.is_empty() {
        Err(CurrentHostEnvironmentError::InvalidObservation)
    } else {
        Ok(cpus)
    }
}

pub(super) fn parse_cpu_info(bytes: &[u8]) -> Result<CpuInventory, CurrentHostEnvironmentError> {
    let text = bounded_utf8(bytes)?;
    let mut logical_cpus = BTreeSet::new();
    let mut physical_cores = BTreeSet::new();
    let mut common_model: Option<String> = None;
    let mut saw_section = false;
    for section in text
        .split("\n\n")
        .filter(|section| !section.trim().is_empty())
    {
        let processor = exact_field(section, "processor")?
            .parse::<u32>()
            .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
        if processor > MAX_CPU_INDEX || !logical_cpus.insert(processor) {
            return Err(CurrentHostEnvironmentError::InvalidObservation);
        }
        let physical_id = exact_field(section, "physical id")?
            .parse::<u32>()
            .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
        let core_id = exact_field(section, "core id")?
            .parse::<u32>()
            .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
        physical_cores.insert((physical_id, core_id));
        let model = normalize_cpu_model(exact_field(section, "model name")?)?;
        if common_model
            .as_ref()
            .is_some_and(|expected| expected != &model)
        {
            return Err(CurrentHostEnvironmentError::ObservationUnavailable);
        }
        common_model.get_or_insert(model);
        saw_section = true;
    }
    let physical_core_count = u32::try_from(physical_cores.len())
        .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
    if !saw_section || physical_core_count == 0 || physical_cores.len() > logical_cpus.len() {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    Ok(CpuInventory {
        cpu_model: common_model.ok_or(CurrentHostEnvironmentError::InvalidObservation)?,
        logical_cpus,
        physical_core_count,
    })
}

pub(super) fn parse_total_memory_mib(bytes: &[u8]) -> Result<u64, CurrentHostEnvironmentError> {
    let text = bounded_utf8(bytes)?;
    let values = text
        .lines()
        .filter_map(|line| line.strip_prefix("MemTotal:"))
        .collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    };
    let mut fields = value.split_ascii_whitespace();
    let kib = fields
        .next()
        .ok_or(CurrentHostEnvironmentError::InvalidObservation)?
        .parse::<u64>()
        .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
    if fields.next() != Some("kB") || fields.next().is_some() {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    let mib = kib / 1_024;
    let rounded = (mib / MEMORY_ROUNDING_GRANULARITY_MIB)
        .checked_mul(MEMORY_ROUNDING_GRANULARITY_MIB)
        .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
    if rounded == 0 {
        Err(CurrentHostEnvironmentError::InvalidObservation)
    } else {
        Ok(rounded)
    }
}

pub(super) fn parse_unified_cgroup_path(
    bytes: &[u8],
) -> Result<Vec<String>, CurrentHostEnvironmentError> {
    let text = bounded_utf8(bytes)?;
    let lines = text.lines().collect::<Vec<_>>();
    let [line] = lines.as_slice() else {
        return Err(CurrentHostEnvironmentError::ObservationUnavailable);
    };
    let path = line
        .strip_prefix("0::/")
        .ok_or(CurrentHostEnvironmentError::ObservationUnavailable)?;
    if path.len() > 4_096 || path.contains('\0') || path.contains('\\') {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let components = path.split('/').collect::<Vec<_>>();
    if components.len() > MAX_CGROUP_DEPTH
        || components
            .iter()
            .any(|value| value.is_empty() || *value == "." || *value == "..")
    {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    Ok(components.into_iter().map(str::to_owned).collect())
}

pub(super) fn cpu_max_is_unlimited(bytes: &[u8]) -> Result<bool, CurrentHostEnvironmentError> {
    let text = exact_one_line(bytes)?;
    let (quota, period) = text
        .split_once(' ')
        .filter(|(quota, period)| {
            !quota.is_empty()
                && !period.is_empty()
                && !quota.bytes().any(|byte| byte.is_ascii_whitespace())
                && !period.bytes().any(|byte| byte.is_ascii_whitespace())
        })
        .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
    let period = period
        .parse::<u64>()
        .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
    if period == 0 {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    if quota == "max" {
        Ok(true)
    } else {
        let quota = quota
            .parse::<u64>()
            .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
        if quota == 0 {
            Err(CurrentHostEnvironmentError::InvalidObservation)
        } else {
            Ok(false)
        }
    }
}

pub(super) fn memory_max_is_unlimited(bytes: &[u8]) -> Result<bool, CurrentHostEnvironmentError> {
    let value = exact_one_line(bytes)?;
    if value == "max" {
        Ok(true)
    } else {
        value
            .parse::<u64>()
            .map(|_limit| false)
            .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)
    }
}

pub(super) fn cgroup_type_is_domain(bytes: &[u8]) -> Result<bool, CurrentHostEnvironmentError> {
    match exact_one_line(bytes)? {
        "domain" => Ok(true),
        "domain threaded" | "domain invalid" | "threaded" => Ok(false),
        _ => Err(CurrentHostEnvironmentError::InvalidObservation),
    }
}

pub(super) fn read_bounded_stream(
    reader: impl Read,
    maximum_bytes: usize,
) -> Result<Vec<u8>, CurrentHostEnvironmentError> {
    let limit = u64::try_from(maximum_bytes)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(CurrentHostEnvironmentError::ObservationUnavailable)?;
    let mut bytes = Vec::new();
    reader
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    } else {
        Ok(bytes)
    }
}

fn exact_field<'a>(section: &'a str, name: &str) -> Result<&'a str, CurrentHostEnvironmentError> {
    let mut values = section.lines().filter_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then_some(value.trim())
    });
    let value = values
        .next()
        .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
    if value.is_empty() || values.next().is_some() {
        Err(CurrentHostEnvironmentError::InvalidObservation)
    } else {
        Ok(value)
    }
}

fn normalize_cpu_model(value: &str) -> Result<String, CurrentHostEnvironmentError> {
    if !value.is_ascii() {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    let normalized = value.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        Err(CurrentHostEnvironmentError::InvalidObservation)
    } else {
        Ok(normalized)
    }
}

fn parse_cpu_index(value: &str) -> Result<u32, CurrentHostEnvironmentError> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(CurrentHostEnvironmentError::InvalidObservation);
    }
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value <= MAX_CPU_INDEX)
        .ok_or(CurrentHostEnvironmentError::InvalidObservation)
}

fn bounded_utf8(bytes: &[u8]) -> Result<&str, CurrentHostEnvironmentError> {
    std::str::from_utf8(bytes).map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)
}

fn exact_one_line(bytes: &[u8]) -> Result<&str, CurrentHostEnvironmentError> {
    let text = bounded_utf8(bytes)?;
    let value = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if value.is_empty() || value.contains('\n') || value.contains('\r') {
        Err(CurrentHostEnvironmentError::InvalidObservation)
    } else {
        Ok(value)
    }
}
