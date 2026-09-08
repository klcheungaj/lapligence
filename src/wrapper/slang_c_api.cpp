#include "slang_c_api.h"

#include <algorithm>
#include <bit>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <deque>
#include <exception>
#include <filesystem>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "slang/analysis/AnalysisManager.h"
#include "slang/ast/Compilation.h"
#include "slang/ast/Scope.h"
#include "slang/ast/SemanticFacts.h"
#include "slang/ast/symbols/CompilationUnitSymbols.h"
#include "slang/ast/symbols/InstanceSymbols.h"
#include "slang/ast/symbols/ParameterSymbols.h"
#include "slang/ast/types/Type.h"
#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/diagnostics/DiagnosticClient.h"
#include "slang/numeric/ConstantValue.h"
#include "slang/parsing/Preprocessor.h"
#include "slang/syntax/SyntaxTree.h"
#include "slang/text/SourceManager.h"
#include "slang/util/Bag.h"

using namespace slang;
using namespace slang::ast;

namespace {

constexpr uint64_t kDefaultMaxSources = 256;
constexpr uint64_t kHardMaxSources = 4096;
constexpr uint64_t kDefaultMaxSourceBytes = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxSourceBytes = 512 * 1024 * 1024;
constexpr uint64_t kDefaultMaxDiagnostics = 10000;
constexpr uint64_t kHardMaxDiagnostics = 100000;
constexpr uint64_t kDefaultMaxInstances = 100000;
constexpr uint64_t kHardMaxInstances = 1000000;
constexpr uint64_t kDefaultMaxParameters = 500000;
constexpr uint64_t kHardMaxParameters = 2000000;
constexpr uint64_t kDefaultMaxTypes = 100000;
constexpr uint64_t kHardMaxTypes = 1000000;
constexpr uint64_t kDefaultMaxValueBits = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxValueBits = 512 * 1024 * 1024;
constexpr uint64_t kDefaultMaxRelatedDiagnostics = 80000;
constexpr uint64_t kHardMaxRelatedDiagnostics = 800000;
constexpr uint64_t kDefaultMaxOutputBytes = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxOutputBytes = 512 * 1024 * 1024;
constexpr uint64_t kHardMaxDefines = 4096;
constexpr uint64_t kHardMaxTopModules = 4096;
constexpr uint64_t kHardMaxIncludeDirs = 4096;
constexpr uint64_t kHardMaxParameterOverrides = 4096;
constexpr uint64_t kHardMaxConfigBytes = 4 * 1024 * 1024;

class BridgeFailure final : public std::runtime_error {
public:
  BridgeFailure(uint32_t status, std::string message)
      : std::runtime_error(std::move(message)), status(status) {}
  uint32_t status;
};

uint64_t effectiveLimit(uint64_t requested, uint64_t defaultValue,
                        uint64_t hardLimit) {
  return std::min(requested == 0 ? defaultValue : requested, hardLimit);
}

std::string_view checkedView(LlgSlangString value, const char* field) {
  if (value.len != 0 && value.data == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        std::string(field) + " has a null data pointer");
  if (value.len > static_cast<uint64_t>(std::numeric_limits<size_t>::max()))
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        std::string(field) + " is too large");
  return {reinterpret_cast<const char*>(value.data), static_cast<size_t>(value.len)};
}

void addChecked(uint64_t& total, uint64_t amount, uint64_t limit,
                const char* description) {
  if (amount > limit || total > limit - amount)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        std::string(description) + " limit exceeded");
  total += amount;
}

} // namespace

struct LlgSlangError {
  uint32_t status = LLG_SLANG_STATUS_INTERNAL_ERROR;
  std::string message;
  bool is_static = false;
};

struct LlgSlangSnapshot {
  uint32_t flags = 0;
  uint64_t output_bytes = 0;
  uint64_t output_byte_limit = kDefaultMaxOutputBytes;
  std::deque<std::string> strings;
  std::vector<LlgSlangFile> files;
  std::vector<LlgSlangDiagnostic> diagnostics;
  std::vector<LlgSlangRelatedDiagnostic> related;
  std::vector<LlgSlangInstance> instances;
  std::vector<LlgSlangParameter> parameters;
  std::vector<LlgSlangType> types;
  std::vector<LlgSlangConstant> constants;
  std::vector<uint64_t> value_words;
};

namespace {

LlgSlangError oomError{LLG_SLANG_STATUS_INTERNAL_ERROR,
                       "unable to allocate native error details", true};

LlgSlangError* makeError(uint32_t status, std::string_view message) noexcept {
  try {
    auto result = std::make_unique<LlgSlangError>();
    result->status = status;
    result->message.assign(message);
    return result.release();
  }
  catch (...) {
    return &oomError;
  }
}

LlgSlangString storeString(LlgSlangSnapshot& snapshot, std::string_view value) {
  if (value.empty())
    return {nullptr, 0};
  addChecked(snapshot.output_bytes, value.size(), snapshot.output_byte_limit,
             "export byte");
  auto& stored = snapshot.strings.emplace_back(value);
  return {reinterpret_cast<const uint8_t*>(stored.data()),
          static_cast<uint64_t>(stored.size())};
}

void chargeRecord(LlgSlangSnapshot& snapshot, uint64_t bytes) {
  addChecked(snapshot.output_bytes, bytes, snapshot.output_byte_limit,
             "export byte");
}

struct Capture {
  LlgSlangSnapshot& output;
  const SourceManager& sourceManager;
  const LlgSlangLimits& limits;
  std::vector<std::pair<BufferID, uint64_t>> fileIds;
  std::vector<std::pair<const Type*, uint64_t>> typeIds;
  uint64_t valueBits = 0;

  uint64_t maxDiagnostics() const {
    return effectiveLimit(limits.max_diagnostics, kDefaultMaxDiagnostics,
                          kHardMaxDiagnostics);
  }
  uint64_t maxInstances() const {
    return effectiveLimit(limits.max_instances, kDefaultMaxInstances,
                          kHardMaxInstances);
  }
  uint64_t maxParameters() const {
    return effectiveLimit(limits.max_parameters, kDefaultMaxParameters,
                          kHardMaxParameters);
  }
  uint64_t maxTypes() const {
    return effectiveLimit(limits.max_types, kDefaultMaxTypes, kHardMaxTypes);
  }
  uint64_t maxValueBits() const {
    return effectiveLimit(limits.max_value_bits, kDefaultMaxValueBits,
                          kHardMaxValueBits);
  }
  uint64_t maxRelatedDiagnostics() const {
    return effectiveLimit(limits.max_related_diagnostics,
                          kDefaultMaxRelatedDiagnostics,
                          kHardMaxRelatedDiagnostics);
  }

  SourceLocation physicalLocation(SourceLocation location) const {
    if (!location.valid())
      return {};
    return sourceManager.getFullyExpandedLoc(location);
  }

  uint64_t findFile(BufferID buffer) const {
    for (const auto& [candidate, id] : fileIds) {
      if (candidate == buffer)
        return id;
    }
    return LLG_SLANG_INVALID_ID;
  }

  LlgSlangSourceRange span(SourceLocation location, uint64_t length = 0) const {
    if (location.valid() && sourceManager.isMacroLoc(location))
      return span(sourceManager.getExpansionRange(location));
    location = physicalLocation(location);
    const uint64_t file = location.valid() ? findFile(location.buffer())
                                           : LLG_SLANG_INVALID_ID;
    if (file == LLG_SLANG_INVALID_ID)
      return {LLG_SLANG_INVALID_ID, 0, 0};
    const uint64_t start = location.offset();
    const uint64_t fileLen = output.files[static_cast<size_t>(file)].byte_len;
    return {file, std::min(start, fileLen),
            std::min(start > fileLen ? fileLen : start + std::min(length, fileLen - start),
                     fileLen)};
  }

  LlgSlangSourceRange span(SourceRange range) const {
    SourceLocation start = physicalLocation(range.start());
    SourceLocation end = physicalLocation(range.end());
    if (!start.valid() || !end.valid() || start.buffer() != end.buffer())
      return span(start);
    const uint64_t file = findFile(start.buffer());
    if (file == LLG_SLANG_INVALID_ID)
      return {LLG_SLANG_INVALID_ID, 0, 0};
    const uint64_t fileLen = output.files[static_cast<size_t>(file)].byte_len;
    const uint64_t begin = std::min<uint64_t>(start.offset(), fileLen);
    const uint64_t finish = std::clamp<uint64_t>(end.offset(), begin, fileLen);
    return {file, begin, finish};
  }

  void addRelated(SourceRange range, std::string_view message = {}) {
    if (output.related.size() >= maxRelatedDiagnostics())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "related diagnostic limit exceeded");
    chargeRecord(output, sizeof(LlgSlangRelatedDiagnostic));
    output.related.push_back({span(range), storeString(output, message)});
  }

  void addRelated(SourceLocation location, std::string_view message = {}) {
    if (output.related.size() >= maxRelatedDiagnostics())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "related diagnostic limit exceeded");
    chargeRecord(output, sizeof(LlgSlangRelatedDiagnostic));
    output.related.push_back({span(location), storeString(output, message)});
  }

  uint64_t addDiagnostic(const ReportedDiagnostic& diag, uint32_t provider,
                         const DiagnosticEngine& engine) {
    if (output.diagnostics.size() >= maxDiagnostics())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "diagnostic limit exceeded");

    const uint64_t relatedStart = output.related.size();
    LlgSlangSourceRange primary = span(diag.location);
    for (const SourceRange& range : diag.ranges) {
      const LlgSlangSourceRange candidate = span(range);
      if (candidate.file_id == primary.file_id && candidate.start <= primary.start &&
          candidate.end >= primary.end && candidate.end > candidate.start) {
        primary = candidate;
        break;
      }
    }
    for (const SourceRange& range : diag.ranges) {
      if (span(range).start != primary.start || span(range).end != primary.end ||
          span(range).file_id != primary.file_id)
        addRelated(range);
    }
    for (SourceLocation expansion : diag.expansionLocs)
      addRelated(sourceManager.getFullyOriginalLoc(expansion));
    if (diag.shouldShowIncludeStack && diag.location.valid()) {
      SourceLocation includedFrom = sourceManager.getIncludedFrom(diag.location.buffer());
      while (includedFrom.valid()) {
        addRelated(includedFrom);
        includedFrom = sourceManager.getIncludedFrom(includedFrom.buffer());
      }
    }

    const Diagnostic& original = diag.originalDiagnostic;
    chargeRecord(output, sizeof(LlgSlangDiagnostic));
    output.diagnostics.push_back({
        provider,
        static_cast<uint32_t>(diag.severity),
        static_cast<uint32_t>(original.code.getSubsystem()),
        original.code.getCode(),
        storeString(output, toString(original.code)),
        storeString(output, engine.getOptionName(original.code)),
        storeString(output, diag.formattedMessage),
        primary,
        relatedStart,
        static_cast<uint64_t>(output.related.size()) - relatedStart,
    });
    return output.diagnostics.size() - 1;
  }


  uint64_t type(const Type& input) {
    const Type& canonical = input.getCanonicalType();
    for (const auto& [existing, id] : typeIds) {
      if (existing == &canonical)
        return id;
    }
    if (output.types.size() >= maxTypes())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "type limit exceeded");

    uint32_t kind = LLG_SLANG_TYPE_OTHER;
    if (canonical.isIntegral())
      kind = LLG_SLANG_TYPE_INTEGRAL;
    else if (canonical.isFloating())
      kind = LLG_SLANG_TYPE_FLOATING;
    else if (canonical.isString())
      kind = LLG_SLANG_TYPE_STRING;
    else if (canonical.isAggregate())
      kind = LLG_SLANG_TYPE_AGGREGATE;

    uint32_t flags = 0;
    if (canonical.isSigned())
      flags |= LLG_SLANG_TYPE_SIGNED;
    if (canonical.isFourState())
      flags |= LLG_SLANG_TYPE_FOUR_STATE;
    if (canonical.isFixedSize())
      flags |= LLG_SLANG_TYPE_FIXED_SIZE;

    const uint64_t id = output.types.size();
    typeIds.emplace_back(&canonical, id);
    chargeRecord(output, sizeof(LlgSlangType));
    output.types.push_back({id, kind, flags, canonical.getBitWidth(),
                            storeString(output, input.toString())});
    return id;
  }

  uint64_t constant(const ConstantValue& value) {
    const uint64_t id = output.constants.size();
    LlgSlangConstant result{};
    result.value_word_start = LLG_SLANG_INVALID_ID;
    result.unknown_word_start = LLG_SLANG_INVALID_ID;

    if (value.isInteger()) {
      const SVInt& integer = value.integer();
      result.kind = LLG_SLANG_VALUE_INTEGER;
      result.is_signed = integer.isSigned() ? 1u : 0u;
      result.bit_width = integer.getBitWidth();
      addChecked(valueBits, result.bit_width, maxValueBits(), "constant bit");
      result.word_count = (result.bit_width + 63) / 64;
      if (result.word_count > std::numeric_limits<uint64_t>::max() /
                                  (2 * sizeof(uint64_t)))
        throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                            "constant word byte size overflow");
      chargeRecord(output, result.word_count * 2 * sizeof(uint64_t));
      result.value_word_start = output.value_words.size();
      output.value_words.resize(output.value_words.size() + result.word_count, 0);
      result.unknown_word_start = output.value_words.size();
      output.value_words.resize(output.value_words.size() + result.word_count, 0);
      for (uint64_t bit = 0; bit < result.bit_width; bit++) {
        const logic_t digit = integer[static_cast<int32_t>(bit)];
        const uint64_t mask = uint64_t{1} << (bit % 64);
        if (digit.isUnknown()) {
          output.value_words[result.unknown_word_start + bit / 64] |= mask;
          if (exactlyEqual(digit, logic_t::z))
            output.value_words[result.value_word_start + bit / 64] |= mask;
        }
        else if (static_cast<bool>(digit)) {
          output.value_words[result.value_word_start + bit / 64] |= mask;
        }
      }
    }
    else if (value.isReal()) {
      result.kind = LLG_SLANG_VALUE_REAL;
      result.bit_width = 64;
      result.real_bits = std::bit_cast<uint64_t>(value.real().v);
    }
    else if (value.isShortReal()) {
      result.kind = LLG_SLANG_VALUE_SHORT_REAL;
      result.bit_width = 32;
      result.real_bits = std::bit_cast<uint32_t>(value.shortReal().v);
    }
    else if (value.isString()) {
      result.kind = LLG_SLANG_VALUE_STRING;
      result.text = storeString(output, value.str());
    }
    else if (value.bad()) {
      result.kind = LLG_SLANG_VALUE_NONE;
    }
    else {
      result.kind = LLG_SLANG_VALUE_OTHER;
      result.text = storeString(output, "<unsupported constant>");
    }

    chargeRecord(output, sizeof(LlgSlangConstant));
    output.constants.push_back(result);
    return id;
  }

  void parameters(const InstanceSymbol& instance, uint64_t instanceId) {
    for (const ParameterSymbolBase* base : instance.body.getParameters()) {
      if (output.parameters.size() >= maxParameters())
        throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                            "parameter limit exceeded");

      const Symbol& symbol = base->symbol;
      uint32_t kind;
      uint64_t typeId;
      uint64_t constantId = LLG_SLANG_INVALID_ID;
      if (symbol.kind == SymbolKind::Parameter) {
        const auto& parameter = symbol.as<ParameterSymbol>();
        kind = LLG_SLANG_PARAMETER_VALUE;
        typeId = type(parameter.getType());
        constantId = constant(parameter.getValue());
      }
      else if (symbol.kind == SymbolKind::TypeParameter) {
        const auto& parameter = symbol.as<TypeParameterSymbol>();
        kind = LLG_SLANG_PARAMETER_TYPE;
        typeId = type(parameter.getTypeAlias());
      }
      else {
        continue;
      }

      uint32_t flags = 0;
      if (base->isLocalParam())
        flags |= LLG_SLANG_PARAMETER_LOCAL;
      if (base->isPortParam())
        flags |= LLG_SLANG_PARAMETER_PORT;
      chargeRecord(output, sizeof(LlgSlangParameter));
      output.parameters.push_back({
          instanceId, kind, flags, storeString(output, symbol.name),
          span(symbol.location, symbol.name.size()), typeId, constantId});
    }
  }

  void scanScope(const Scope& scope, uint64_t parentId) {
    for (const Symbol& symbol : scope.members()) {
      if (symbol.kind == SymbolKind::Instance) {
        instance(symbol.as<InstanceSymbol>(), parentId);
      }
      else if (const Scope* nested = symbol.as_if<Scope>()) {
        scanScope(*nested, parentId);
      }
    }
  }

  void instance(const InstanceSymbol& input, uint64_t parentId) {
    if (output.instances.size() >= maxInstances())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "instance limit exceeded");

    uint32_t kind = LLG_SLANG_INSTANCE_UNKNOWN;
    switch (input.getDefinition().definitionKind) {
      case DefinitionKind::Module: kind = LLG_SLANG_INSTANCE_MODULE; break;
      case DefinitionKind::Interface: kind = LLG_SLANG_INSTANCE_INTERFACE; break;
      case DefinitionKind::Program: kind = LLG_SLANG_INSTANCE_PROGRAM; break;
    }

    const uint64_t id = output.instances.size();
    const uint64_t parameterStart = output.parameters.size();
    chargeRecord(output, sizeof(LlgSlangInstance));
    output.instances.push_back({
        id, parentId, kind, 0, storeString(output, input.name),
        storeString(output, input.getDefinition().name),
        span(input.location, input.name.size()), parameterStart, 0});
    parameters(input, id);
    output.instances[static_cast<size_t>(id)].parameter_count =
        output.parameters.size() - parameterStart;
    scanScope(input.body, id);
  }
};

class CaptureClient final : public DiagnosticClient {
public:
  CaptureClient(Capture& capture, uint32_t provider)
      : capture(capture), provider(provider) {}

  void issue(DiagnosticEngine& diagnosticEngine, const Diagnostic& diagnostic) {
    root = &diagnostic;
    rootIndex = LLG_SLANG_INVALID_ID;
    if (diagnostic.isError())
      capture.output.flags |= LLG_SLANG_SNAPSHOT_HAS_ERRORS;
    diagnosticEngine.issue(diagnostic);
    root = nullptr;
  }

  void report(const ReportedDiagnostic& diagnostic) override {
    if (&diagnostic.originalDiagnostic == root) {
      if (diagnostic.severity == DiagnosticSeverity::Error ||
          diagnostic.severity == DiagnosticSeverity::Fatal)
        capture.output.flags |= LLG_SLANG_SNAPSHOT_HAS_ERRORS;
      rootIndex = capture.addDiagnostic(diagnostic, provider, *engine);
      return;
    }
    if (rootIndex == LLG_SLANG_INVALID_ID)
      return;

    capture.addRelated(diagnostic.location, diagnostic.formattedMessage);
    for (const SourceRange& range : diagnostic.ranges)
      capture.addRelated(range, diagnostic.formattedMessage);
    auto& parent = capture.output.diagnostics[static_cast<size_t>(rootIndex)];
    parent.related_count = capture.output.related.size() - parent.related_start;
  }

private:
  Capture& capture;
  uint32_t provider;
  const Diagnostic* root = nullptr;
  uint64_t rootIndex = LLG_SLANG_INVALID_ID;
};

class FreezeGuard {
public:
  explicit FreezeGuard(Compilation& compilation) : compilation(compilation) {
    compilation.freeze();
  }
  ~FreezeGuard() { compilation.unfreeze(); }
  FreezeGuard(const FreezeGuard&) = delete;
  FreezeGuard& operator=(const FreezeGuard&) = delete;

private:
  Compilation& compilation;
};

std::unique_ptr<LlgSlangSnapshot> compileImpl(const LlgSlangCompileRequest& request) {
  if (request.abi_version != LLG_SLANG_ABI_VERSION)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unsupported Slang ABI version");
  if (request.flags != 0)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unknown compile request flags");
  if (request.source_count != 0 && request.sources == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "sources has a null pointer");
  if (request.source_count == 0)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "at least one source is required");
  if (request.define_count != 0 && request.defines == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "defines has a null pointer");
  if (request.top_module_count != 0 && request.top_modules == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "top_modules has a null pointer");
  if (request.include_dir_count != 0 && request.include_dirs == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "include_dirs has a null pointer");
  if (request.parameter_override_count != 0 && request.parameter_overrides == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "parameter_overrides has a null pointer");

  const uint64_t maxSources = effectiveLimit(request.limits.max_sources,
      kDefaultMaxSources, kHardMaxSources);
  const uint64_t maxSourceBytes = effectiveLimit(request.limits.max_source_bytes,
      kDefaultMaxSourceBytes, kHardMaxSourceBytes);
  if (request.source_count > maxSources)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "source count limit exceeded");
  if (request.define_count > kHardMaxDefines)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "define count limit exceeded");
  if (request.top_module_count > kHardMaxTopModules)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "top module count limit exceeded");
  if (request.include_dir_count > kHardMaxIncludeDirs)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "include directory count limit exceeded");
  if (request.parameter_override_count > kHardMaxParameterOverrides)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "parameter override count limit exceeded");

  uint64_t sourceBytes = 0;
  uint64_t configBytes = 0;
  std::vector<std::string> sourceNames;
  sourceNames.reserve(static_cast<size_t>(request.source_count));
  std::vector<std::string> sourcePaths;
  sourcePaths.reserve(static_cast<size_t>(request.source_count));
  std::vector<std::string_view> sourceTexts;
  sourceTexts.reserve(static_cast<size_t>(request.source_count));
  for (uint64_t i = 0; i < request.source_count; i++) {
    const auto& input = request.sources[i];
    if ((input.flags & ~LLG_SLANG_SOURCE_COMPILATION_UNIT) != 0 || input.reserved != 0)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "unknown source flags or nonzero reserved field");
    const std::string_view name = checkedView(input.name, "source name");
    const std::string_view text = checkedView(input.text, "source text");
    if (name.empty() || name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "source names must be nonempty and contain no NUL bytes");
    addChecked(sourceBytes, name.size(), maxSourceBytes, "source byte");
    addChecked(sourceBytes, text.size(), maxSourceBytes, "source byte");
    const std::string normalized =
        std::filesystem::path(name).lexically_normal().generic_string();
    if (std::find(sourcePaths.begin(), sourcePaths.end(), normalized) != sourcePaths.end())
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "source names must be unique after lexical normalization");
    sourceNames.emplace_back(name);
    sourcePaths.push_back(normalized);
    sourceTexts.push_back(text);
  }

  std::vector<std::string> predefines;
  predefines.reserve(static_cast<size_t>(request.define_count));
  for (uint64_t i = 0; i < request.define_count; i++) {
    const auto& input = request.defines[i];
    if (input.has_value > 1 || input.reserved != 0)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "invalid define flags");
    const std::string_view name = checkedView(input.name, "define name");
    const std::string_view value = checkedView(input.value, "define value");
    if (name.empty() || name.find('\0') != std::string_view::npos ||
        value.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "define text is empty or contains a NUL byte");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes, "configuration byte");
    addChecked(configBytes, value.size(), kHardMaxConfigBytes, "configuration byte");
    predefines.emplace_back(name);
    if (input.has_value) {
      predefines.back().push_back('=');
      predefines.back().append(value);
    }
  }

  CompilationOptions compilationOptions;
  compilationOptions.errorLimit = static_cast<uint32_t>(std::min<uint64_t>(
      effectiveLimit(request.limits.max_diagnostics, kDefaultMaxDiagnostics,
                     kHardMaxDiagnostics),
      std::numeric_limits<uint32_t>::max()));
  compilationOptions.maxConstantSize = effectiveLimit(
      request.limits.max_value_bits, kDefaultMaxValueBits, kHardMaxValueBits);
  compilationOptions.maxInstanceArray = static_cast<uint32_t>(std::min<uint64_t>(
      effectiveLimit(request.limits.max_instances, kDefaultMaxInstances,
                     kHardMaxInstances),
      std::numeric_limits<uint32_t>::max()));
  std::vector<std::string> topNames;
  topNames.reserve(static_cast<size_t>(request.top_module_count));
  for (uint64_t i = 0; i < request.top_module_count; i++) {
    const std::string_view name = checkedView(request.top_modules[i], "top module name");
    if (name.empty() || name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "top module names must be nonempty and contain no NUL bytes");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes, "configuration byte");
    topNames.emplace_back(name);
  }
  for (const std::string& name : topNames)
    compilationOptions.topModules.emplace(name);

  compilationOptions.paramOverrides.reserve(
      static_cast<size_t>(request.parameter_override_count));
  for (uint64_t i = 0; i < request.parameter_override_count; i++) {
    const auto& input = request.parameter_overrides[i];
    if (input.has_value != 1 || input.reserved != 0)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "parameter overrides require a value");
    const std::string_view name = checkedView(input.name, "parameter override name");
    const std::string_view value = checkedView(input.value, "parameter override value");
    if (name.empty() || name.find('\0') != std::string_view::npos ||
        value.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "parameter override text is empty or contains a NUL byte");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes,
               "configuration byte");
    addChecked(configBytes, value.size(), kHardMaxConfigBytes,
               "configuration byte");
    compilationOptions.paramOverrides.emplace_back(name);
    compilationOptions.paramOverrides.back().push_back('=');
    compilationOptions.paramOverrides.back().append(value);
  }

  std::vector<std::filesystem::path> includeDirs;
  includeDirs.reserve(static_cast<size_t>(request.include_dir_count));
  for (uint64_t i = 0; i < request.include_dir_count; i++) {
    const std::string_view name = checkedView(request.include_dirs[i],
                                              "include directory");
    if (name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "include directories must contain no NUL bytes");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes,
               "configuration byte");
    includeDirs.emplace_back(std::filesystem::path(name).lexically_normal());
  }

  SourceManager sourceManager;
  sourceManager.setDisableProximatePaths(true);
  sourceManager.setCacheOnlyReads(true);

  std::vector<SourceBuffer> buffers;
  buffers.reserve(static_cast<size_t>(request.source_count));
  for (uint64_t i = 0; i < request.source_count; i++)
    buffers.push_back(sourceManager.assignText(sourcePaths[i], sourceTexts[i]));

  parsing::PreprocessorOptions preprocessorOptions;
  preprocessorOptions.predefines = std::move(predefines);
  preprocessorOptions.additionalIncludePaths = std::move(includeDirs);
  Bag parseOptions;
  parseOptions.set(std::move(preprocessorOptions));
  Bag compileOptions;
  compileOptions.set(std::move(compilationOptions));
  Compilation compilation(compileOptions);
  bool anyCompilationUnit = false;
  for (uint64_t i = 0; i < request.source_count; i++) {
    if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) == 0)
      continue;
    anyCompilationUnit = true;
    compilation.addSyntaxTree(syntax::SyntaxTree::fromBuffer(
        buffers[static_cast<size_t>(i)], sourceManager, parseOptions));
  }
  if (!anyCompilationUnit)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "at least one compilation unit source is required");

  auto output = std::make_unique<LlgSlangSnapshot>();
  output->output_byte_limit = effectiveLimit(request.limits.max_output_bytes,
      kDefaultMaxOutputBytes, kHardMaxOutputBytes);
  Capture capture{*output, sourceManager, request.limits};
  for (uint64_t i = 0; i < request.source_count; i++) {
    chargeRecord(*output, sizeof(LlgSlangFile));
    output->files.push_back({i, storeString(*output, sourceNames[i]),
                             static_cast<uint64_t>(sourceTexts[i].size())});
    capture.fileIds.emplace_back(buffers[static_cast<size_t>(i)].id, i);
  }

  // A cached include gets a new BufferID that refers to the same admitted file.
  // Associate every physical buffer with its input file before copying ranges.
  for (BufferID buffer : sourceManager.getAllBuffers()) {
    if (!sourceManager.isFileLoc(SourceLocation(buffer, 0)))
      continue;
    const auto path = sourceManager.getFullPath(buffer).generic_string();
    for (uint64_t i = 0; i < request.source_count; i++) {
      if (path == sourcePaths[static_cast<size_t>(i)]) {
        capture.fileIds.emplace_back(buffer, i);
        break;
      }
    }
  }

  const Diagnostics& compilationDiagnostics = compilation.getAllDiagnostics();
  DiagnosticEngine engine(sourceManager);
  auto compilationClient = std::make_shared<CaptureClient>(
      capture, LLG_SLANG_DIAG_COMPILATION);
  engine.addClient(compilationClient);
  Diagnostics pragmaDiagnostics = engine.setMappingsFromPragmas();
  for (const Diagnostic& diagnostic : pragmaDiagnostics)
    compilationClient->issue(engine, diagnostic);
  for (const Diagnostic& diagnostic : compilationDiagnostics)
    compilationClient->issue(engine, diagnostic);

  const RootSymbol& root = compilation.getRootNoFinalize();
  for (const InstanceSymbol* top : root.topInstances)
    capture.instance(*top, LLG_SLANG_INVALID_ID);

  {
    FreezeGuard frozen(compilation);
    analysis::AnalysisOptions analysisOptions;
    analysisOptions.flags |= analysis::AnalysisFlags::CheckUnused |
                             analysis::AnalysisFlags::CheckShadow;
    analysis::AnalysisManager manager(analysisOptions);
    manager.analyze(compilation);
    Diagnostics diagnostics = manager.getDiagnostics();
    diagnostics.sort(sourceManager);
    engine.clearClients();
    auto analysisClient = std::make_shared<CaptureClient>(
        capture, LLG_SLANG_DIAG_ANALYSIS);
    engine.addClient(analysisClient);
    for (const Diagnostic& diagnostic : diagnostics)
      analysisClient->issue(engine, diagnostic);
    output->flags |= LLG_SLANG_SNAPSHOT_ANALYSIS_RAN;
  }

  return output;
}

template<typename T>
const T* dataOrNull(const std::vector<T>& values) {
  return values.empty() ? nullptr : values.data();
}

} // namespace

extern "C" uint32_t llg_slang_compile(const LlgSlangCompileRequest* request,
                                       LlgSlangSnapshot** out_snapshot,
                                       LlgSlangError** out_error) {
  if (out_snapshot)
    *out_snapshot = nullptr;
  if (out_error)
    *out_error = nullptr;
  if (!request || !out_snapshot || !out_error) {
    if (out_error)
      *out_error = makeError(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                             "request and output pointers are required");
    return LLG_SLANG_STATUS_INVALID_ARGUMENT;
  }

  try {
    *out_snapshot = compileImpl(*request).release();
    return LLG_SLANG_STATUS_OK;
  }
  catch (const BridgeFailure& error) {
    *out_error = makeError(error.status, error.what());
    return error.status;
  }
  catch (const std::exception& error) {
    *out_error = makeError(LLG_SLANG_STATUS_FRONTEND_ERROR, error.what());
    return LLG_SLANG_STATUS_FRONTEND_ERROR;
  }
  catch (...) {
    *out_error = makeError(LLG_SLANG_STATUS_INTERNAL_ERROR,
                           "unknown native frontend failure");
    return LLG_SLANG_STATUS_INTERNAL_ERROR;
  }
}

extern "C" uint32_t llg_slang_snapshot_view(const LlgSlangSnapshot* snapshot,
                                             LlgSlangSnapshotView* out_view,
                                             LlgSlangError** out_error) {
  if (out_error)
    *out_error = nullptr;
  if (!snapshot || !out_view || !out_error) {
    if (out_error)
      *out_error = makeError(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                             "snapshot and output pointers are required");
    return LLG_SLANG_STATUS_INVALID_ARGUMENT;
  }

  *out_view = {
      LLG_SLANG_ABI_VERSION,
      snapshot->flags,
      dataOrNull(snapshot->files), static_cast<uint64_t>(snapshot->files.size()),
      dataOrNull(snapshot->diagnostics), static_cast<uint64_t>(snapshot->diagnostics.size()),
      dataOrNull(snapshot->related), static_cast<uint64_t>(snapshot->related.size()),
      dataOrNull(snapshot->instances), static_cast<uint64_t>(snapshot->instances.size()),
      dataOrNull(snapshot->parameters), static_cast<uint64_t>(snapshot->parameters.size()),
      dataOrNull(snapshot->types), static_cast<uint64_t>(snapshot->types.size()),
      dataOrNull(snapshot->constants), static_cast<uint64_t>(snapshot->constants.size()),
      dataOrNull(snapshot->value_words), static_cast<uint64_t>(snapshot->value_words.size()),
  };
  return LLG_SLANG_STATUS_OK;
}

extern "C" uint32_t llg_slang_error_view(const LlgSlangError* error,
                                          LlgSlangErrorView* out_view) {
  if (!error || !out_view)
    return LLG_SLANG_STATUS_INVALID_ARGUMENT;
  out_view->status = error->status;
  out_view->reserved = 0;
  out_view->message = error->message.empty()
      ? LlgSlangString{nullptr, 0}
      : LlgSlangString{reinterpret_cast<const uint8_t*>(error->message.data()),
                       static_cast<uint64_t>(error->message.size())};
  return LLG_SLANG_STATUS_OK;
}

extern "C" void llg_slang_snapshot_destroy(LlgSlangSnapshot* snapshot) {
  delete snapshot;
}

extern "C" void llg_slang_error_destroy(LlgSlangError* error) {
  if (error && !error->is_static)
    delete error;
}
