import {
  ProtocolSelectionSchema,
  VersionRangeSchema,
  create,
} from "@ym-connect/protocol";
import { negotiateCapabilities } from "./capabilities.js";
import { CompatibilityError } from "./errors.js";
import {
  SUPPORTED_PROTOCOL_RANGE,
  normalizeProtocolVersion,
} from "./version.js";

export function compareProtocolVersions(leftValue, rightValue) {
  const left = normalizeProtocolVersion(leftValue);
  const right = normalizeProtocolVersion(rightValue);
  return left.major - right.major || left.minor - right.minor || left.patch - right.patch;
}

export function normalizeVersionRange(value = SUPPORTED_PROTOCOL_RANGE) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("version range must be an object");
  }
  const minimum = normalizeProtocolVersion(value.minimum);
  const maximum = normalizeProtocolVersion(value.maximum);
  if (compareProtocolVersions(minimum, maximum) > 0) {
    throw new CompatibilityError("version range minimum exceeds maximum");
  }
  return create(VersionRangeSchema, { minimum, maximum });
}

export function versionInRange(versionValue, rangeValue) {
  const version = normalizeProtocolVersion(versionValue);
  const range = normalizeVersionRange(rangeValue);
  return (
    compareProtocolVersions(version, range.minimum) >= 0 &&
    compareProtocolVersions(version, range.maximum) <= 0
  );
}

export function intersectVersionRanges(leftValue, rightValue) {
  const left = normalizeVersionRange(leftValue);
  const right = normalizeVersionRange(rightValue);
  const minimum = compareProtocolVersions(left.minimum, right.minimum) >= 0 ? left.minimum : right.minimum;
  const maximum = compareProtocolVersions(left.maximum, right.maximum) <= 0 ? left.maximum : right.maximum;
  if (compareProtocolVersions(minimum, maximum) > 0) return undefined;
  return create(VersionRangeSchema, { minimum, maximum });
}

export function selectProtocolVersion(localRangeValue, remoteRangeValue) {
  const intersection = intersectVersionRanges(localRangeValue, remoteRangeValue);
  if (!intersection) return undefined;
  return intersection.maximum;
}

export function assertProtocolCompatible(localRangeValue, remoteRangeValue) {
  const selected = selectProtocolVersion(localRangeValue, remoteRangeValue);
  if (!selected) {
    throw new CompatibilityError("protocol version ranges do not overlap");
  }
  return selected;
}

export function negotiateProtocol(localRangeValue, remoteRangeValue, localCapabilities, remoteCapabilities) {
  const localRange = normalizeVersionRange(localRangeValue);
  const remoteRange = normalizeVersionRange(remoteRangeValue);
  const selectedVersion = selectProtocolVersion(localRange, remoteRange);
  const capabilities = negotiateCapabilities(localCapabilities, remoteCapabilities, { allowUnknown: true });
  return create(ProtocolSelectionSchema, {
    localRange,
    remoteRange,
    selectedVersion,
    capabilities,
    compatible: selectedVersion !== undefined && capabilities.missingRequired.length === 0,
  });
}
