#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

config_path = File.expand_path("../.github/dependabot.yml", __dir__)
expected = [
  ["cargo", "/"],
  ["cargo", "/py"],
  ["cargo", "/crates/wasm"],
  ["cargo", "/crates/cloud"],
  ["cargo", "/containers/verify-runner"],
  ["npm", "/sdk/ts"],
  ["npm", "/sdk/mcp"],
  ["npm", "/web"],
  ["npm", "/docs/site"],
  ["npm", "/services/cloud"],
  ["npm", "/tools/license-issuer"],
  ["github-actions", "/"]
].freeze

def fail_policy(message)
  warn "dependabot policy: #{message}"
  exit 1
end

config = YAML.safe_load(
  File.read(config_path, encoding: "UTF-8"),
  permitted_classes: [],
  permitted_symbols: [],
  aliases: false
)
fail_policy("expected a YAML mapping") unless config.is_a?(Hash)
fail_policy("expected version: 2") unless config["version"] == 2

updates = config["updates"]
fail_policy("updates must be a list") unless updates.is_a?(Array)

found = updates.map.with_index do |entry, index|
  fail_policy("updates[#{index}] must be a mapping") unless entry.is_a?(Hash)

  ecosystem = entry["package-ecosystem"]
  directory = entry["directory"]
  key = [ecosystem, directory]
  fail_policy("#{key}: open-pull-requests-limit must equal 2") unless entry["open-pull-requests-limit"] == 2

  groups = entry["groups"]
  fail_policy("#{key}: groups must contain only non-major") unless groups.is_a?(Hash) && groups.keys == ["non-major"]
  non_major = groups["non-major"]
  unless non_major.is_a?(Hash) && non_major["patterns"] == ["*"] && non_major["update-types"] == ["minor", "patch"]
    fail_policy("#{key}: expected the canonical minor/patch group")
  end

  key
end

fail_policy("duplicate ecosystem/directory entry") unless found.length == found.uniq.length
missing = expected - found
extra = found - expected
fail_policy("entry mismatch; missing=#{missing.inspect}, extra=#{extra.inspect}") unless missing.empty? && extra.empty?

puts "dependabot policy: ok (#{found.length} bounded version-update roots)"
