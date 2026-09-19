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
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <string_view>
#include <tuple>
#include <utility>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include "slang/analysis/AnalysisManager.h"
#include "slang/ast/ASTVisitor.h"
#include "slang/ast/Compilation.h"
#include "slang/ast/Lookup.h"
#include "slang/ast/Scope.h"
#include "slang/ast/SemanticFacts.h"
#include "slang/ast/symbols/CompilationUnitSymbols.h"
#include "slang/ast/symbols/ClassSymbols.h"
#include "slang/ast/symbols/InstanceSymbols.h"
#include "slang/ast/symbols/MemberSymbols.h"
#include "slang/ast/symbols/ParameterSymbols.h"
#include "slang/ast/symbols/VariableSymbols.h"
#include "slang/ast/types/AllTypes.h"
#include "slang/ast/types/Type.h"
#include "slang/diagnostics/AnalysisDiags.h"
#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/diagnostics/DiagnosticClient.h"
#include "slang/driver/UserDefinedSubroutine.h"
#include "slang/numeric/ConstantValue.h"
#include "slang/parsing/Parser.h"
#include "slang/parsing/Preprocessor.h"
#include "slang/parsing/LexerFacts.h"
#include "slang/syntax/AllSyntax.h"
#include "slang/syntax/SyntaxTree.h"
#include "slang/syntax/SyntaxVisitor.h"
#include "slang/text/SourceManager.h"
#include "slang/util/Bag.h"
#include "slang/util/SmallVector.h"

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
constexpr uint64_t kDefaultMaxConstants = 1000000;
constexpr uint64_t kHardMaxConstants = 4000000;
constexpr uint64_t kDefaultMaxTypes = 100000;
constexpr uint64_t kHardMaxTypes = 1000000;
constexpr uint64_t kDefaultMaxValueBits = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxValueBits = 512 * 1024 * 1024;
constexpr uint64_t kDefaultMaxRelatedDiagnostics = 80000;
constexpr uint64_t kHardMaxRelatedDiagnostics = 800000;
constexpr uint64_t kDefaultMaxOutputBytes = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxOutputBytes = 512 * 1024 * 1024;

// Named-event identity is carried by the terminal event type even when the
// declaration adds one or more unpacked dimensions. Keep this test in the
// frontend capture so the owned database receives one declaration identity
// for both scalar events and legal event arrays.
bool isNamedEventType(const Type& type) {
  const Type& canonical = type.getCanonicalType();
  if (canonical.isEvent())
    return true;
  if (canonical.kind == SymbolKind::FixedSizeUnpackedArrayType) {
    const auto& array = canonical.as<FixedSizeUnpackedArrayType>();
    return isNamedEventType(array.elementType);
  }
  if (canonical.kind == SymbolKind::DynamicArrayType) {
    const auto& array = canonical.as<DynamicArrayType>();
    return isNamedEventType(array.elementType);
  }
  if (canonical.kind == SymbolKind::AssociativeArrayType) {
    const auto& array = canonical.as<AssociativeArrayType>();
    return isNamedEventType(array.elementType);
  }
  if (canonical.kind == SymbolKind::QueueType) {
    const auto& array = canonical.as<QueueType>();
    return isNamedEventType(array.elementType);
  }
  return false;
}
constexpr uint64_t kDefaultMaxSemanticNodes = 1000000;
constexpr uint64_t kHardMaxSemanticNodes = 4000000;
constexpr uint64_t kDefaultMaxSemanticEdges = 4000000;
constexpr uint64_t kHardMaxSemanticEdges = 16000000;
constexpr uint64_t kDefaultMaxLexicalTokens = 4000000;
constexpr uint64_t kHardMaxLexicalTokens = 16000000;
constexpr uint64_t kDefaultMaxTypeRanges = 1000000;
constexpr uint64_t kHardMaxTypeRanges = 4000000;
constexpr uint64_t kDefaultMaxTypeMembers = 1000000;
constexpr uint64_t kHardMaxTypeMembers = 4000000;
constexpr uint64_t kHardMaxDefines = 4096;
constexpr uint64_t kHardMaxTopModules = 4096;
constexpr uint64_t kHardMaxIncludeDirs = 4096;
constexpr uint64_t kHardMaxParameterOverrides = 4096;
constexpr uint64_t kHardMaxSystemSubroutines = 4096;
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
  if (value.len == 0)
    return {};
  return {reinterpret_cast<const char*>(value.data), static_cast<size_t>(value.len)};
}

void addChecked(uint64_t& total, uint64_t amount, uint64_t limit,
                const char* description) {
  if (amount > limit || total > limit - amount)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        std::string(description) + " limit exceeded");
  total += amount;
}

struct EditionPolicy {
  LanguageVersion languageVersion;
  parsing::KeywordVersion keywordVersion;
  uint32_t snapshotFlag;
};

EditionPolicy editionPolicy(uint32_t flags) {
  constexpr uint32_t editionMask =
      LLG_SLANG_COMPILE_EDITION_VERILOG_2001 |
      LLG_SLANG_COMPILE_EDITION_SYSTEMVERILOG_2009;
  switch (flags & editionMask) {
    case 0:
    case LLG_SLANG_COMPILE_EDITION_SYSTEMVERILOG_2009:
      return {LanguageVersion::v1800_2017, parsing::KeywordVersion::v1800_2009,
              LLG_SLANG_SNAPSHOT_EDITION_SYSTEMVERILOG_2009};
    case LLG_SLANG_COMPILE_EDITION_VERILOG_2001:
      return {LanguageVersion::v1364_2005, parsing::KeywordVersion::v1364_2001,
              LLG_SLANG_SNAPSHOT_EDITION_VERILOG_2001};
    default:
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "multiple language editions were selected");
  }
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
  std::vector<LlgSlangSemanticNode> semantic_nodes;
  std::vector<LlgSlangSemanticEdge> semantic_edges;
  std::vector<LlgSlangLexicalToken> lexical_tokens;
  std::vector<LlgSlangTypeRange> type_ranges;
  std::vector<LlgSlangTypeMember> type_members;
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
  if (bytes > snapshot.output_byte_limit ||
      snapshot.output_bytes > snapshot.output_byte_limit - bytes) {
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
        "export byte limit exceeded (bytes=" + std::to_string(snapshot.output_bytes) +
        ", nodes=" + std::to_string(snapshot.semantic_nodes.size()) +
        ", types=" + std::to_string(snapshot.types.size()) +
        ", constants=" + std::to_string(snapshot.constants.size()) +
        ", instances=" + std::to_string(snapshot.instances.size()) + ")");
  }
  addChecked(snapshot.output_bytes, bytes, snapshot.output_byte_limit,
             "export byte");
}

uint32_t diagnosticSubsystem(DiagSubsystem subsystem) {
  switch (subsystem) {
    case DiagSubsystem::Invalid: return LLG_SLANG_DIAG_SUBSYSTEM_INVALID;
    case DiagSubsystem::General: return LLG_SLANG_DIAG_SUBSYSTEM_GENERAL;
    case DiagSubsystem::Lexer: return LLG_SLANG_DIAG_SUBSYSTEM_LEXER;
    case DiagSubsystem::Numeric: return LLG_SLANG_DIAG_SUBSYSTEM_NUMERIC;
    case DiagSubsystem::Preprocessor:
      return LLG_SLANG_DIAG_SUBSYSTEM_PREPROCESSOR;
    case DiagSubsystem::Parser: return LLG_SLANG_DIAG_SUBSYSTEM_PARSER;
    case DiagSubsystem::Declarations:
      return LLG_SLANG_DIAG_SUBSYSTEM_DECLARATIONS;
    case DiagSubsystem::Expressions:
      return LLG_SLANG_DIAG_SUBSYSTEM_EXPRESSIONS;
    case DiagSubsystem::Statements: return LLG_SLANG_DIAG_SUBSYSTEM_STATEMENTS;
    case DiagSubsystem::Types: return LLG_SLANG_DIAG_SUBSYSTEM_TYPES;
    case DiagSubsystem::Lookup: return LLG_SLANG_DIAG_SUBSYSTEM_LOOKUP;
    case DiagSubsystem::SysFuncs: return LLG_SLANG_DIAG_SUBSYSTEM_SYSFUNCS;
    case DiagSubsystem::ConstEval: return LLG_SLANG_DIAG_SUBSYSTEM_CONST_EVAL;
    case DiagSubsystem::Compilation:
      return LLG_SLANG_DIAG_SUBSYSTEM_COMPILATION;
    case DiagSubsystem::Analysis: return LLG_SLANG_DIAG_SUBSYSTEM_ANALYSIS;
    case DiagSubsystem::Meta: return LLG_SLANG_DIAG_SUBSYSTEM_META;
    case DiagSubsystem::Driver: return LLG_SLANG_DIAG_SUBSYSTEM_DRIVER;
    case DiagSubsystem::Tidy: return LLG_SLANG_DIAG_SUBSYSTEM_TIDY;
    case DiagSubsystem::Netlist: return LLG_SLANG_DIAG_SUBSYSTEM_NETLIST;
  }
  return LLG_SLANG_DIAG_SUBSYSTEM_INVALID;
}

uint32_t diagnosticSeverity(DiagnosticSeverity severity) {
  switch (severity) {
    case DiagnosticSeverity::Ignored: return LLG_SLANG_SEVERITY_IGNORED;
    case DiagnosticSeverity::Note: return LLG_SLANG_SEVERITY_NOTE;
    case DiagnosticSeverity::Warning: return LLG_SLANG_SEVERITY_WARNING;
    case DiagnosticSeverity::Error: return LLG_SLANG_SEVERITY_ERROR;
    case DiagnosticSeverity::Fatal: return LLG_SLANG_SEVERITY_FATAL;
  }
  throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                      "unknown Slang diagnostic severity");
}

struct Capture {
  struct LexicalBinding {
    LlgSlangSourceRange range;
    uint64_t semantic_id;
    uint32_t role;
    uint32_t kind;
  };

  struct GenvarLexicalScope {
    LlgSlangSourceRange range;
    std::string name;
    uint64_t semantic_id;
  };

  LlgSlangSnapshot& output;
  const SourceManager& sourceManager;
  const LlgSlangLimits& limits;
  std::vector<std::pair<BufferID, uint64_t>> fileIds;
  std::unordered_map<const Type*, uint64_t> typeIds;
  // Independently declared equivalent virtual-interface types need one owned
  // identity. Canonical pointers alone are only unique for nominal classes.
  std::unordered_map<const DefinitionSymbol*, std::vector<const VirtualInterfaceType*>>
      virtualInterfaceTypes;
  std::unordered_map<const void*, uint64_t> semanticIds;
  std::vector<std::vector<LlgSlangSemanticEdge>> pendingEdges;
  std::vector<LexicalBinding> lexicalBindings;
  std::vector<LlgSlangSourceRange> connectionActualRanges;
  std::vector<GenvarLexicalScope> genvarLexicalScopes;
  std::unordered_map<uint64_t, uint64_t> lexicalTargetAliases;
  std::unordered_map<const syntax::SyntaxNode*, std::vector<uint64_t>>
      sourceIdentityGroups;
  std::unordered_set<const ParameterSymbol*> overriddenParameters;
  uint64_t valueBits = 0;
  uint64_t semanticEdgeCount = 0;
  bool declarationOnly = false;

  Capture(LlgSlangSnapshot& output, const SourceManager& sourceManager,
          const LlgSlangLimits& limits, bool declarationOnly = false)
      : output(output), sourceManager(sourceManager), limits(limits),
        declarationOnly(declarationOnly) {}

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
  uint64_t maxConstants() const {
    return effectiveLimit(limits.max_constants, kDefaultMaxConstants,
                          kHardMaxConstants);
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
  uint64_t maxSemanticNodes() const {
    return effectiveLimit(limits.max_semantic_nodes,
                          kDefaultMaxSemanticNodes, kHardMaxSemanticNodes);
  }
  uint64_t maxSemanticEdges() const {
    return effectiveLimit(limits.max_semantic_edges,
                          kDefaultMaxSemanticEdges, kHardMaxSemanticEdges);
  }
  uint64_t maxLexicalTokens() const {
    return effectiveLimit(limits.max_lexical_tokens,
                          kDefaultMaxLexicalTokens, kHardMaxLexicalTokens);
  }
  uint64_t maxTypeRanges() const {
    return effectiveLimit(limits.max_type_ranges, kDefaultMaxTypeRanges,
                          kHardMaxTypeRanges);
  }
  uint64_t maxTypeMembers() const {
    return effectiveLimit(limits.max_type_members, kDefaultMaxTypeMembers,
                          kHardMaxTypeMembers);
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

  std::string_view sourceText(SourceRange range) const {
    const SourceLocation start = physicalLocation(range.start());
    const SourceLocation end = physicalLocation(range.end());
    if (!start.valid() || !end.valid() || start.buffer() != end.buffer() ||
        findFile(start.buffer()) == LLG_SLANG_INVALID_ID)
      return {};
    const std::string_view text = sourceManager.getSourceText(start.buffer());
    const size_t begin = std::min(start.offset(), text.size());
    const size_t finish = std::clamp(end.offset(), begin, text.size());
    return text.substr(begin, finish - begin);
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
        diagnosticSeverity(diag.severity),
        diagnosticSubsystem(original.code.getSubsystem()),
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

  void addIntegrationDiagnostic(std::string_view name, std::string message) {
    if (output.diagnostics.size() >= maxDiagnostics())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "diagnostic limit exceeded");
    chargeRecord(output, sizeof(LlgSlangDiagnostic));
    output.diagnostics.push_back({
        LLG_SLANG_DIAG_COMPILATION,
        LLG_SLANG_SEVERITY_ERROR,
        LLG_SLANG_DIAG_SUBSYSTEM_COMPILATION,
        1,
        storeString(output, name),
        {},
        storeString(output, message),
        {LLG_SLANG_INVALID_ID, 0, 0},
        output.related.size(),
        0,
    });
    output.flags |= LLG_SLANG_SNAPSHOT_HAS_ERRORS;
  }


  uint64_t type(const Type& input) {
    const Type& canonical = input.getCanonicalType();
    if (auto it = typeIds.find(&canonical); it != typeIds.end())
      return it->second;
    if (canonical.kind == SymbolKind::VirtualInterfaceType) {
      const auto& vif = canonical.as<VirtualInterfaceType>();
      auto& candidates = virtualInterfaceTypes[&vif.iface.getDefinition()];
      for (const auto* candidate : candidates) {
        if (canonical.isEquivalent(*candidate)) {
          const uint64_t id = typeIds.at(candidate);
          typeIds.emplace(&canonical, id);
          return id;
        }
      }
    }
    if (output.types.size() >= maxTypes())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "type limit exceeded");

    uint32_t kind = LLG_SLANG_TYPE_OTHER;
    switch (canonical.kind) {
      case SymbolKind::EnumType: kind = LLG_SLANG_TYPE_ENUM; break;
      case SymbolKind::PackedArrayType: kind = LLG_SLANG_TYPE_PACKED_ARRAY; break;
      case SymbolKind::FixedSizeUnpackedArrayType:
        kind = LLG_SLANG_TYPE_FIXED_UNPACKED_ARRAY;
        break;
      case SymbolKind::DynamicArrayType: kind = LLG_SLANG_TYPE_DYNAMIC_ARRAY; break;
      case SymbolKind::AssociativeArrayType:
        kind = LLG_SLANG_TYPE_ASSOCIATIVE_ARRAY;
        break;
      case SymbolKind::QueueType: kind = LLG_SLANG_TYPE_QUEUE; break;
      case SymbolKind::PackedStructType: kind = LLG_SLANG_TYPE_PACKED_STRUCT; break;
      case SymbolKind::PackedUnionType: kind = LLG_SLANG_TYPE_PACKED_UNION; break;
      case SymbolKind::UnpackedStructType:
        kind = LLG_SLANG_TYPE_UNPACKED_STRUCT;
        break;
      case SymbolKind::UnpackedUnionType:
        kind = LLG_SLANG_TYPE_UNPACKED_UNION;
        break;
      case SymbolKind::ClassType: kind = LLG_SLANG_TYPE_CLASS; break;
      case SymbolKind::CHandleType: kind = LLG_SLANG_TYPE_CHANDLE; break;
      case SymbolKind::EventType: kind = LLG_SLANG_TYPE_EVENT; break;
      case SymbolKind::VoidType: kind = LLG_SLANG_TYPE_VOID; break;
      case SymbolKind::VirtualInterfaceType:
        kind = LLG_SLANG_TYPE_VIRTUAL_INTERFACE;
        break;
      default:
        if (canonical.isIntegral())
          kind = LLG_SLANG_TYPE_INTEGRAL;
        else if (canonical.isFloating())
          kind = LLG_SLANG_TYPE_FLOATING;
        else if (canonical.isString())
          kind = LLG_SLANG_TYPE_STRING;
        else if (canonical.isAggregate())
          kind = LLG_SLANG_TYPE_AGGREGATE;
        break;
    }

    uint32_t flags = 0;
    if (canonical.isSigned())
      flags |= LLG_SLANG_TYPE_SIGNED;
    if (canonical.isFourState())
      flags |= LLG_SLANG_TYPE_FOUR_STATE;
    if (canonical.isFixedSize())
      flags |= LLG_SLANG_TYPE_FIXED_SIZE;

    const uint64_t id = output.types.size();
    typeIds.emplace(&canonical, id);
    if (canonical.kind == SymbolKind::VirtualInterfaceType) {
      const auto& vif = canonical.as<VirtualInterfaceType>();
      virtualInterfaceTypes[&vif.iface.getDefinition()].push_back(&vif);
    }
    chargeRecord(output, sizeof(LlgSlangType));
    output.types.push_back({id, kind, flags, canonical.getBitWidth(),
                            storeString(output, input.toString()),
                            LLG_SLANG_INVALID_ID, LLG_SLANG_INVALID_ID,
                            0, 0, 0, 0});

    uint64_t elementType = LLG_SLANG_INVALID_ID;
    uint64_t indexType = LLG_SLANG_INVALID_ID;
    std::optional<LlgSlangTypeRange> range;
    std::vector<const FieldSymbol*> fields;
    switch (canonical.kind) {
      case SymbolKind::PackedArrayType: {
        const auto& array = canonical.as<PackedArrayType>();
        elementType = type(array.elementType);
        range = LlgSlangTypeRange{array.range.left, array.range.right,
                                  LLG_SLANG_TYPE_RANGE_PACKED, 0};
        break;
      }
      case SymbolKind::FixedSizeUnpackedArrayType: {
        const auto& array = canonical.as<FixedSizeUnpackedArrayType>();
        elementType = type(array.elementType);
        range = LlgSlangTypeRange{array.range.left, array.range.right,
                                  LLG_SLANG_TYPE_RANGE_UNPACKED, 0};
        break;
      }
      case SymbolKind::DynamicArrayType:
        elementType = type(canonical.as<DynamicArrayType>().elementType);
        break;
      case SymbolKind::AssociativeArrayType: {
        const auto& array = canonical.as<AssociativeArrayType>();
        elementType = type(array.elementType);
        if (array.indexType)
          indexType = type(*array.indexType);
        break;
      }
      case SymbolKind::QueueType: {
        const auto& queue = canonical.as<QueueType>();
        elementType = type(queue.elementType);
        if (queue.maxBound)
          range = LlgSlangTypeRange{0, queue.maxBound,
                                    LLG_SLANG_TYPE_RANGE_QUEUE_BOUND, 0};
        break;
      }
      case SymbolKind::EnumType:
        elementType = type(canonical.as<EnumType>().baseType);
        break;
      case SymbolKind::PackedStructType:
        for (const FieldSymbol& field :
             canonical.as<PackedStructType>().membersOfType<FieldSymbol>())
          fields.push_back(&field);
        break;
      case SymbolKind::PackedUnionType:
        for (const FieldSymbol& field :
             canonical.as<PackedUnionType>().membersOfType<FieldSymbol>())
          fields.push_back(&field);
        break;
      case SymbolKind::UnpackedStructType:
        for (const FieldSymbol* field : canonical.as<UnpackedStructType>().fields)
          fields.push_back(field);
        break;
      case SymbolKind::UnpackedUnionType:
        for (const FieldSymbol* field : canonical.as<UnpackedUnionType>().fields)
          fields.push_back(field);
        break;
      default: break;
    }

    uint64_t rangeStart = output.type_ranges.size();
    if (range) {
      if (output.type_ranges.size() >= maxTypeRanges())
        throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                            "type range limit exceeded");
      chargeRecord(output, sizeof(LlgSlangTypeRange));
      output.type_ranges.push_back(*range);
    }

    std::vector<LlgSlangTypeMember> members;
    members.reserve(fields.size());
    for (const FieldSymbol* field : fields) {
      if (members.size() >= maxTypeMembers() -
                                std::min<uint64_t>(output.type_members.size(),
                                                   maxTypeMembers()))
        throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                            "type member limit exceeded");
      members.push_back({storeString(output, field->name), type(field->getType()),
                         field->bitOffset, field->getType().getBitWidth()});
    }
    const uint64_t memberStart = output.type_members.size();
    chargeRecord(output, members.size() * sizeof(LlgSlangTypeMember));
    output.type_members.insert(output.type_members.end(), members.begin(), members.end());

    auto& result = output.types[static_cast<size_t>(id)];
    result.element_type_id = elementType;
    result.index_type_id = indexType;
    result.range_start = rangeStart;
    result.range_count = range ? 1 : 0;
    result.member_start = memberStart;
    result.member_count = members.size();
    return id;
  }

  uint64_t constant(const ConstantValue& value) {
    if (output.constants.size() >= maxConstants())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "constant limit exceeded");
    chargeRecord(output, sizeof(LlgSlangConstant));
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
        const auto* syntax = parameter.getSyntax();
        const auto* overrides = instance.body.hierarchyOverrideNode;
        if (instance.isTopLevel() && syntax &&
            ((overrides && overrides->paramOverrides.find(syntax) !=
                              overrides->paramOverrides.end()) ||
             parameter.isOverridden()))
          overriddenParameters.insert(&parameter);
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

  uint64_t ensureSemantic(const void* identity) {
    if (auto it = semanticIds.find(identity); it != semanticIds.end())
      return it->second;
    if (output.semantic_nodes.size() >= maxSemanticNodes())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "semantic node limit exceeded");
    const uint64_t id = output.semantic_nodes.size();
    semanticIds.emplace(identity, id);
    pendingEdges.emplace_back();
    chargeRecord(output, sizeof(LlgSlangSemanticNode));
    output.semantic_nodes.push_back({
        id, LLG_SLANG_INVALID_ID, LLG_SLANG_SEMANTIC_UNSUPPORTED,
        LLG_SLANG_SUBKIND_NONE, LLG_SLANG_OP_NONE, 0, {}, {}, {},
        {LLG_SLANG_INVALID_ID, 0, 0}, LLG_SLANG_INVALID_ID,
        LLG_SLANG_INVALID_ID, LLG_SLANG_INVALID_ID, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, LLG_SLANG_ASSERTION_RANGE_UNBOUNDED, 0});
    return id;
  }

  uint64_t newSyntheticSemantic() {
    if (output.semantic_nodes.size() >= maxSemanticNodes())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "semantic node limit exceeded");
    const uint64_t id = output.semantic_nodes.size();
    pendingEdges.emplace_back();
    chargeRecord(output, sizeof(LlgSlangSemanticNode));
    output.semantic_nodes.push_back({
        id, LLG_SLANG_INVALID_ID, LLG_SLANG_SEMANTIC_UNSUPPORTED,
        LLG_SLANG_SUBKIND_NONE, LLG_SLANG_OP_NONE, 0, {}, {}, {},
        {LLG_SLANG_INVALID_ID, 0, 0}, LLG_SLANG_INVALID_ID,
        LLG_SLANG_INVALID_ID, LLG_SLANG_INVALID_ID, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, LLG_SLANG_ASSERTION_RANGE_UNBOUNDED, 0});
    return id;
  }

  void semanticEdge(uint64_t source, uint32_t role, uint64_t target,
                    uint32_t index = 0) {
    if (source == LLG_SLANG_INVALID_ID || target == LLG_SLANG_INVALID_ID)
      return;
    for (const auto& edge : pendingEdges[static_cast<size_t>(source)]) {
      if (edge.role != role || edge.index != index)
        continue;
      if (edge.target_id == target)
        return;
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "conflicting semantic edge role and index");
    }
    if (semanticEdgeCount >= maxSemanticEdges())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "semantic edge limit exceeded");
    chargeRecord(output, sizeof(LlgSlangSemanticEdge));
    pendingEdges[static_cast<size_t>(source)].push_back({role, index, target});
    semanticEdgeCount++;
  }

  void semanticChild(uint64_t parent, uint64_t child) {
    uint32_t index = 0;
    for (const auto& edge : pendingEdges[static_cast<size_t>(parent)]) {
      if (edge.role == LLG_SLANG_EDGE_CHILD)
        index = std::max(index, edge.index + 1);
    }
    semanticEdge(parent, LLG_SLANG_EDGE_CHILD, child, index);
  }

  void semanticRole(uint64_t source, const void* targetIdentity, uint32_t role,
                    uint32_t index = 0) {
    const uint64_t target = ensureSemantic(targetIdentity);
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    const auto existing = std::find_if(edges.begin(), edges.end(),
        [=](const LlgSlangSemanticEdge& edge) {
          return edge.role == role && edge.index == index;
        });
    if (existing != edges.end()) {
      if (existing->target_id != target)
        throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                            "conflicting semantic edge role and index");
      const size_t oldSize = edges.size();
      edges.erase(std::remove_if(edges.begin(), edges.end(),
          [=](const LlgSlangSemanticEdge& edge) {
            return edge.role == LLG_SLANG_EDGE_CHILD &&
                   edge.target_id == target;
          }), edges.end());
      semanticEdgeCount -= oldSize - edges.size();
      return;
    }
    for (auto& edge : edges) {
      if (edge.target_id == target && edge.role == LLG_SLANG_EDGE_CHILD) {
        edge.role = role;
        edge.index = index;
        return;
      }
    }
    semanticEdge(source, role, target, index);
  }

  // Attach a checked SequenceRange to a SequenceConcat operand. The edge
  // remains an ordinary owned identity edge; scalar metadata is copied
  // alongside it and never exposes a native Slang object to Rust.
  void semanticSequenceRole(uint64_t source, const void* targetIdentity,
                            uint32_t role, uint32_t index,
                            const SequenceRange& range) {
    semanticRole(source, targetIdentity, role, index);
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    auto it = std::find_if(edges.begin(), edges.end(), [=](const auto& edge) {
      return edge.role == role && edge.index == index;
    });
    if (it == edges.end())
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "sequence edge metadata has no edge");
    it->sequence_delay_min = range.min;
    it->sequence_delay_max = range.max.value_or(LLG_SLANG_ASSERTION_RANGE_UNBOUNDED);
    it->sequence_delay_valid = 1;
  }

  void replaceChildRoles(uint64_t source, uint32_t role) {
    uint32_t index = 0;
    for (auto& edge : pendingEdges[static_cast<size_t>(source)]) {
      if (edge.role == LLG_SLANG_EDGE_CHILD) {
        edge.role = role;
        edge.index = index++;
      }
    }
  }

  void removeChildEdges(uint64_t source) {
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    const size_t oldSize = edges.size();
    edges.erase(std::remove_if(edges.begin(), edges.end(),
        [](const LlgSlangSemanticEdge& edge) {
          return edge.role == LLG_SLANG_EDGE_CHILD;
        }), edges.end());
    semanticEdgeCount -= oldSize - edges.size();
  }

  void finalizeSemanticEdges() {
    output.semantic_edges.reserve(static_cast<size_t>(semanticEdgeCount));
    for (size_t i = 0; i < pendingEdges.size(); i++) {
      auto& node = output.semantic_nodes[i];
      node.edge_start = output.semantic_edges.size();
      node.edge_count = pendingEdges[i].size();
      output.semantic_edges.insert(output.semantic_edges.end(),
                                   pendingEdges[i].begin(), pendingEdges[i].end());
    }
  }

  void sourceIdentity(const syntax::SyntaxNode* syntaxNode, uint64_t semanticId) {
    if (syntaxNode)
      sourceIdentityGroups[syntaxNode].push_back(semanticId);
  }

  void markOverriddenParameters() {
    // Overrides belong to an elaborated parameter, not every instance that
    // happens to share its source declaration (syntax node).
    for (const auto* parameter : overriddenParameters) {
      auto it = semanticIds.find(parameter);
      if (it == semanticIds.end())
        continue;
      auto& node = output.semantic_nodes[static_cast<size_t>(it->second)];
      if (node.kind == LLG_SLANG_SEMANTIC_PARAMETER)
        node.auxiliary = 1;
    }
  }

  void finalizeSourceIdentities() {
    for (const auto& [syntaxNode, ids] : sourceIdentityGroups) {
      (void)syntaxNode;
      if (ids.size() < 2)
        continue;
      uint64_t representative = *std::min_element(ids.begin(), ids.end());
      for (uint64_t id : ids) {
        const auto& node = output.semantic_nodes[static_cast<size_t>(id)];
        if (node.kind == LLG_SLANG_SEMANTIC_VARIABLE &&
            node.subkind == LLG_SLANG_VARIABLE_GENVAR) {
          representative = id;
          break;
        }
      }
      for (uint64_t id : ids) {
        lexicalTargetAliases.insert_or_assign(id, representative);
        if (id != representative) {
          semanticEdge(id, LLG_SLANG_EDGE_SOURCE_IDENTITY, representative);
        }
      }
    }
    for (auto& scope : genvarLexicalScopes) {
      scope.semantic_id = canonicalLexicalTarget(scope.semantic_id);
    }
  }

  uint64_t canonicalLexicalTarget(uint64_t semanticId) const {
    for (size_t remaining = lexicalTargetAliases.size(); remaining != 0;
         remaining--) {
      const auto alias = lexicalTargetAliases.find(semanticId);
      if (alias == lexicalTargetAliases.end() || alias->second == semanticId)
        break;
      semanticId = alias->second;
    }
    return semanticId;
  }

  void lexicalBinding(parsing::Token token, uint64_t semanticId,
                      uint32_t role, uint32_t kind = LLG_SLANG_LEXICAL_UNKNOWN) {
    if (lexicalBindings.size() >= maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "lexical binding limit exceeded");
    lexicalBindings.push_back({span(token.range()), semanticId, role, kind});
  }

  void connectionActual(SourceRange range) {
    if (connectionActualRanges.size() >= maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "connection actual range limit exceeded");
    const auto physical = span(range);
    if (physical.file_id != LLG_SLANG_INVALID_ID)
      connectionActualRanges.push_back(physical);
  }

  void genvarLexicalScope(SourceRange range, std::string_view name,
                          uint64_t semanticId) {
    if (genvarLexicalScopes.size() >= maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "genvar lexical scope limit exceeded");
    const auto physical = span(range);
    if (physical.file_id != LLG_SLANG_INVALID_ID)
      genvarLexicalScopes.push_back(
          {physical, std::string(name), semanticId});
  }
};

uint32_t semanticSymbolKind(SymbolKind kind) {
  switch (kind) {
    case SymbolKind::Definition: return LLG_SLANG_SEMANTIC_DEFINITION;
    case SymbolKind::Instance:
    case SymbolKind::InstanceArray: return LLG_SLANG_SEMANTIC_INSTANCE;
    case SymbolKind::InstanceBody: return LLG_SLANG_SEMANTIC_SCOPE;
    case SymbolKind::Package: return LLG_SLANG_SEMANTIC_PACKAGE;
    case SymbolKind::ClassType:
    case SymbolKind::GenericClassDef: return LLG_SLANG_SEMANTIC_CLASS;
    case SymbolKind::GenerateBlock:
    case SymbolKind::GenerateBlockArray: return LLG_SLANG_SEMANTIC_GENERATE_SCOPE;
    case SymbolKind::Port:
    case SymbolKind::MultiPort:
    case SymbolKind::InterfacePort: return LLG_SLANG_SEMANTIC_PORT;
    case SymbolKind::Modport:
    case SymbolKind::ModportPort: return LLG_SLANG_SEMANTIC_MODPORT;
    case SymbolKind::Net: return LLG_SLANG_SEMANTIC_NET;
    case SymbolKind::NetAlias: return LLG_SLANG_SEMANTIC_NET_ALIAS;
    case SymbolKind::Variable:
    case SymbolKind::Genvar:
    case SymbolKind::Field:
    case SymbolKind::ClassProperty:
    case SymbolKind::Iterator:
    case SymbolKind::PatternVar:
    case SymbolKind::LocalAssertionVar: return LLG_SLANG_SEMANTIC_VARIABLE;
    case SymbolKind::NetType: return LLG_SLANG_SEMANTIC_UNSUPPORTED;
    case SymbolKind::Parameter:
    case SymbolKind::TypeParameter:
    case SymbolKind::Specparam: return LLG_SLANG_SEMANTIC_PARAMETER;
    case SymbolKind::ProceduralBlock: return LLG_SLANG_SEMANTIC_PROCESS;
    case SymbolKind::ContinuousAssign: return LLG_SLANG_SEMANTIC_CONTINUOUS_ASSIGN;
    case SymbolKind::Primitive:
    case SymbolKind::PrimitiveInstance:
    case SymbolKind::PrimitivePort: return LLG_SLANG_SEMANTIC_PRIMITIVE;
    case SymbolKind::Subroutine:
    case SymbolKind::MethodPrototype: return LLG_SLANG_SEMANTIC_SUBROUTINE;
    case SymbolKind::FormalArgument: return LLG_SLANG_SEMANTIC_ARGUMENT;
    case SymbolKind::AssertionPort: return LLG_SLANG_SEMANTIC_ARGUMENT;
    case SymbolKind::EnumValue: return LLG_SLANG_SEMANTIC_ENUM_CONSTANT;
    case SymbolKind::Root:
    case SymbolKind::CompilationUnit:
    case SymbolKind::StatementBlock: return LLG_SLANG_SEMANTIC_SCOPE;
    case SymbolKind::ClockingBlock: return LLG_SLANG_SEMANTIC_SCOPE;
    case SymbolKind::ClockVar: return LLG_SLANG_SEMANTIC_VARIABLE;
    default: return LLG_SLANG_SEMANTIC_UNSUPPORTED;
  }
}

uint64_t clockingEdgeCode(EdgeKind edge) {
  switch (edge) {
    case EdgeKind::None: return 0;
    case EdgeKind::PosEdge: return 1;
    case EdgeKind::NegEdge: return 2;
    case EdgeKind::BothEdges: return 3;
  }
  return 0;
}

uint32_t semanticStatementKind(StatementKind kind) {
  switch (kind) {
    case StatementKind::Empty: return LLG_SLANG_STMT_EMPTY;
    case StatementKind::List: return LLG_SLANG_STMT_LIST;
    case StatementKind::Block: return LLG_SLANG_STMT_BLOCK;
    case StatementKind::ExpressionStatement: return LLG_SLANG_STMT_EXPRESSION;
    case StatementKind::VariableDeclaration: return LLG_SLANG_STMT_VARIABLE_DECL;
    case StatementKind::Return: return LLG_SLANG_STMT_RETURN;
    case StatementKind::Continue: return LLG_SLANG_STMT_CONTINUE;
    case StatementKind::Break: return LLG_SLANG_STMT_BREAK;
    case StatementKind::Disable: return LLG_SLANG_STMT_DISABLE;
    case StatementKind::DisableFork: return LLG_SLANG_STMT_DISABLE_FORK;
    case StatementKind::Conditional: return LLG_SLANG_STMT_IF;
    case StatementKind::Case: return LLG_SLANG_STMT_CASE;
    case StatementKind::PatternCase: return LLG_SLANG_STMT_PATTERN_CASE;
    case StatementKind::ForLoop: return LLG_SLANG_STMT_FOR;
    case StatementKind::ForeachLoop: return LLG_SLANG_STMT_FOREACH;
    case StatementKind::RepeatLoop: return LLG_SLANG_STMT_REPEAT;
    case StatementKind::WhileLoop: return LLG_SLANG_STMT_WHILE;
    case StatementKind::DoWhileLoop: return LLG_SLANG_STMT_DO_WHILE;
    case StatementKind::ForeverLoop: return LLG_SLANG_STMT_FOREVER;
    case StatementKind::Timed: return LLG_SLANG_STMT_TIMED;
    case StatementKind::Wait:
    case StatementKind::WaitOrder: return LLG_SLANG_STMT_WAIT;
    case StatementKind::WaitFork: return LLG_SLANG_STMT_WAIT_FORK;
    case StatementKind::EventTrigger: return LLG_SLANG_STMT_EVENT_TRIGGER;
    case StatementKind::ImmediateAssertion: return LLG_SLANG_STMT_IMMEDIATE_ASSERT;
    case StatementKind::ConcurrentAssertion: return LLG_SLANG_STMT_CONCURRENT_ASSERT;
    case StatementKind::ProceduralAssign:
      return LLG_SLANG_STMT_PROCEDURAL_ASSIGN;
    case StatementKind::ProceduralDeassign:
      return LLG_SLANG_STMT_PROCEDURAL_DEASSIGN;
    default: return LLG_SLANG_SUBKIND_NONE;
  }
}

uint32_t semanticAssertionExprKind(AssertionExprKind kind) {
  switch (kind) {
    case AssertionExprKind::Invalid: return LLG_SLANG_ASSERTION_EXPR_INVALID;
    case AssertionExprKind::Simple: return LLG_SLANG_ASSERTION_EXPR_SIMPLE;
    case AssertionExprKind::SequenceConcat:
      return LLG_SLANG_ASSERTION_EXPR_SEQUENCE_CONCAT;
    case AssertionExprKind::SequenceWithMatch:
      return LLG_SLANG_ASSERTION_EXPR_SEQUENCE_WITH_MATCH;
    case AssertionExprKind::Unary: return LLG_SLANG_ASSERTION_EXPR_UNARY;
    case AssertionExprKind::Binary: return LLG_SLANG_ASSERTION_EXPR_BINARY;
    case AssertionExprKind::FirstMatch: return LLG_SLANG_ASSERTION_EXPR_FIRST_MATCH;
    case AssertionExprKind::Clocking: return LLG_SLANG_ASSERTION_EXPR_CLOCKING;
    case AssertionExprKind::StrongWeak: return LLG_SLANG_ASSERTION_EXPR_STRONG_WEAK;
    case AssertionExprKind::Abort: return LLG_SLANG_ASSERTION_EXPR_ABORT;
    case AssertionExprKind::Conditional: return LLG_SLANG_ASSERTION_EXPR_CONDITIONAL;
    case AssertionExprKind::Case: return LLG_SLANG_ASSERTION_EXPR_CASE;
    case AssertionExprKind::DisableIff: return LLG_SLANG_ASSERTION_EXPR_DISABLE_IFF;
  }
  return LLG_SLANG_ASSERTION_EXPR_INVALID;
}

uint32_t semanticUnaryAssertionOperation(UnaryAssertionOperator op) {
  switch (op) {
    case UnaryAssertionOperator::Not: return LLG_SLANG_OP_ASSERTION_NOT;
    case UnaryAssertionOperator::NextTime: return LLG_SLANG_OP_ASSERTION_NEXT_TIME;
    case UnaryAssertionOperator::SNextTime: return LLG_SLANG_OP_ASSERTION_SNEXT_TIME;
    case UnaryAssertionOperator::Always: return LLG_SLANG_OP_ASSERTION_ALWAYS;
    case UnaryAssertionOperator::SAlways: return LLG_SLANG_OP_ASSERTION_SALWAYS;
    case UnaryAssertionOperator::Eventually: return LLG_SLANG_OP_ASSERTION_EVENTUALLY;
    case UnaryAssertionOperator::SEventually: return LLG_SLANG_OP_ASSERTION_SEVENTUALLY;
  }
  return LLG_SLANG_OP_NONE;
}

uint32_t semanticBinaryAssertionOperation(BinaryAssertionOperator op) {
  switch (op) {
    case BinaryAssertionOperator::And: return LLG_SLANG_OP_ASSERTION_AND;
    case BinaryAssertionOperator::Or: return LLG_SLANG_OP_ASSERTION_OR;
    case BinaryAssertionOperator::Intersect:
      return LLG_SLANG_OP_ASSERTION_INTERSECT;
    case BinaryAssertionOperator::Throughout:
      return LLG_SLANG_OP_ASSERTION_THROUGHOUT;
    case BinaryAssertionOperator::Within: return LLG_SLANG_OP_ASSERTION_WITHIN;
    case BinaryAssertionOperator::Iff: return LLG_SLANG_OP_ASSERTION_IFF;
    case BinaryAssertionOperator::Until: return LLG_SLANG_OP_ASSERTION_UNTIL;
    case BinaryAssertionOperator::SUntil: return LLG_SLANG_OP_ASSERTION_SUNTIL;
    case BinaryAssertionOperator::UntilWith:
      return LLG_SLANG_OP_ASSERTION_UNTIL_WITH;
    case BinaryAssertionOperator::SUntilWith:
      return LLG_SLANG_OP_ASSERTION_SUNTIL_WITH;
    case BinaryAssertionOperator::Implies: return LLG_SLANG_OP_ASSERTION_IMPLIES;
    case BinaryAssertionOperator::OverlappedImplication:
      return LLG_SLANG_OP_ASSERTION_OVERLAPPED_IMPLIES;
    case BinaryAssertionOperator::NonOverlappedImplication:
      return LLG_SLANG_OP_ASSERTION_NONOVERLAPPED_IMPLIES;
    case BinaryAssertionOperator::OverlappedFollowedBy:
      return LLG_SLANG_OP_ASSERTION_OVERLAPPED_FOLLOWED_BY;
    case BinaryAssertionOperator::NonOverlappedFollowedBy:
      return LLG_SLANG_OP_ASSERTION_NONOVERLAPPED_FOLLOWED_BY;
  }
  return LLG_SLANG_OP_NONE;
}

uint64_t semanticUniquePriorityCheck(UniquePriorityCheck check) {
  switch (check) {
    case UniquePriorityCheck::None: return LLG_SLANG_UNIQUE_PRIORITY_NONE;
    case UniquePriorityCheck::Unique: return LLG_SLANG_UNIQUE_PRIORITY_UNIQUE;
    case UniquePriorityCheck::Unique0: return LLG_SLANG_UNIQUE_PRIORITY_UNIQUE0;
    case UniquePriorityCheck::Priority: return LLG_SLANG_UNIQUE_PRIORITY_PRIORITY;
  }
  return LLG_SLANG_UNIQUE_PRIORITY_NONE;
}

uint32_t semanticExpressionKind(ExpressionKind kind) {
  switch (kind) {
    case ExpressionKind::IntegerLiteral:
    case ExpressionKind::RealLiteral:
    case ExpressionKind::UnbasedUnsizedIntegerLiteral:
    case ExpressionKind::NullLiteral:
    case ExpressionKind::UnboundedLiteral:
    case ExpressionKind::StringLiteral: return LLG_SLANG_EXPR_LITERAL;
    case ExpressionKind::TimeLiteral: return LLG_SLANG_EXPR_TIME_LITERAL;
    case ExpressionKind::NamedValue:
    case ExpressionKind::HierarchicalValue: return LLG_SLANG_EXPR_REFERENCE;
    case ExpressionKind::UnaryOp: return LLG_SLANG_EXPR_UNARY;
    case ExpressionKind::BinaryOp: return LLG_SLANG_EXPR_BINARY;
    case ExpressionKind::ConditionalOp: return LLG_SLANG_EXPR_CONDITIONAL;
    case ExpressionKind::Inside: return LLG_SLANG_EXPR_INSIDE;
    case ExpressionKind::Assignment: return LLG_SLANG_EXPR_ASSIGNMENT;
    case ExpressionKind::Concatenation:
    case ExpressionKind::Streaming: return LLG_SLANG_EXPR_CONCAT;
    case ExpressionKind::Replication: return LLG_SLANG_EXPR_REPLICATION;
    case ExpressionKind::ElementSelect: return LLG_SLANG_EXPR_ELEMENT_SELECT;
    case ExpressionKind::RangeSelect: return LLG_SLANG_EXPR_RANGE_SELECT;
    case ExpressionKind::MemberAccess: return LLG_SLANG_EXPR_MEMBER_ACCESS;
    case ExpressionKind::Call: return LLG_SLANG_EXPR_CALL;
    case ExpressionKind::Conversion: return LLG_SLANG_EXPR_CONVERSION;
    case ExpressionKind::SimpleAssignmentPattern:
    case ExpressionKind::StructuredAssignmentPattern:
    case ExpressionKind::ReplicatedAssignmentPattern:
      return LLG_SLANG_EXPR_ASSIGNMENT_PATTERN;
    case ExpressionKind::NewArray: return LLG_SLANG_EXPR_NEW_ARRAY;
    case ExpressionKind::NewClass: return LLG_SLANG_EXPR_NEW_CLASS;
    case ExpressionKind::NewCovergroup: return LLG_SLANG_EXPR_NEW_COVERGROUP;
    case ExpressionKind::MinTypMax: return LLG_SLANG_EXPR_MIN_TYP_MAX;
    case ExpressionKind::ValueRange: return LLG_SLANG_EXPR_VALUE_RANGE;
    case ExpressionKind::AssertionInstance: return LLG_SLANG_EXPR_ASSERTION_INSTANCE;
    case ExpressionKind::ClockingEvent: return LLG_SLANG_EXPR_CLOCKING_EVENT;
    default: return LLG_SLANG_SUBKIND_NONE;
  }
}

bool isSystemMethodCall(const CallExpression& expression) {
  if (!expression.isSystemCall() || expression.arguments().empty())
    return false;

  const auto& info = std::get<CallExpression::SystemCallInfo>(expression.subroutine);
  const Expression* receiver = expression.arguments().front();
  const Type& receiverType = receiver->type->getCanonicalType();
  return info.scope->getCompilation().getSystemMethod(
             receiverType.kind, expression.getSubroutineName()) ==
         info.subroutine.get();
}

bool isSuperMethodCall(const CallExpression& expression) {
  if (!expression.syntax ||
      expression.syntax->kind != syntax::SyntaxKind::InvocationExpression)
    return false;

  const auto& invocation =
      expression.syntax->as<syntax::InvocationExpressionSyntax>();
  if (invocation.left->kind != syntax::SyntaxKind::ScopedName)
    return false;

  const auto& scoped = invocation.left->as<syntax::ScopedNameSyntax>();
  return scoped.left->getLastToken().kind == parsing::TokenKind::SuperKeyword;
}

const Expression* unwrapImplicitConversions(const Expression& expression) {
  const Expression* unwrapped = &expression;
  while (unwrapped->kind == ExpressionKind::Conversion) {
    const auto& conversion = unwrapped->as<ConversionExpression>();
    if (!conversion.isImplicit())
      return nullptr;
    unwrapped = &conversion.operand();
  }
  return unwrapped;
}

const Expression* compoundAssignmentSourceRhs(
    const AssignmentExpression& expression) {
  if (!expression.op)
    return &expression.right();

  const Expression* expanded = unwrapImplicitConversions(expression.right());
  if (!expanded)
    return nullptr;
  if (expanded->kind != ExpressionKind::BinaryOp)
    return nullptr;
  const auto& binary = expanded->as<BinaryExpression>();
  const Expression* left = unwrapImplicitConversions(binary.left());
  if (binary.op != *expression.op || !left ||
      left->kind != ExpressionKind::LValueReference) {
    return nullptr;
  }
  return &binary.right();
}

uint32_t semanticUnaryOperation(UnaryOperator op) {
  switch (op) {
    case UnaryOperator::Plus: return LLG_SLANG_OP_PLUS;
    case UnaryOperator::Minus: return LLG_SLANG_OP_MINUS;
    case UnaryOperator::BitwiseNot: return LLG_SLANG_OP_BIT_NOT;
    case UnaryOperator::BitwiseAnd: return LLG_SLANG_OP_BIT_AND;
    case UnaryOperator::BitwiseOr: return LLG_SLANG_OP_BIT_OR;
    case UnaryOperator::BitwiseXor: return LLG_SLANG_OP_BIT_XOR;
    case UnaryOperator::BitwiseNand: return LLG_SLANG_OP_BIT_NAND;
    case UnaryOperator::BitwiseNor: return LLG_SLANG_OP_BIT_NOR;
    case UnaryOperator::BitwiseXnor: return LLG_SLANG_OP_BIT_XNOR;
    case UnaryOperator::LogicalNot: return LLG_SLANG_OP_LOGICAL_NOT;
    case UnaryOperator::Preincrement: return LLG_SLANG_OP_PRE_INCREMENT;
    case UnaryOperator::Predecrement: return LLG_SLANG_OP_PRE_DECREMENT;
    case UnaryOperator::Postincrement: return LLG_SLANG_OP_POST_INCREMENT;
    case UnaryOperator::Postdecrement: return LLG_SLANG_OP_POST_DECREMENT;
  }
  return LLG_SLANG_OP_NONE;
}

uint32_t semanticBinaryOperation(BinaryOperator op) {
  switch (op) {
    case BinaryOperator::Add: return LLG_SLANG_OP_PLUS;
    case BinaryOperator::Subtract: return LLG_SLANG_OP_MINUS;
    case BinaryOperator::Multiply: return LLG_SLANG_OP_MULTIPLY;
    case BinaryOperator::Divide: return LLG_SLANG_OP_DIVIDE;
    case BinaryOperator::Mod: return LLG_SLANG_OP_MODULO;
    case BinaryOperator::Power: return LLG_SLANG_OP_POWER;
    case BinaryOperator::BinaryAnd: return LLG_SLANG_OP_BIT_AND;
    case BinaryOperator::BinaryOr: return LLG_SLANG_OP_BIT_OR;
    case BinaryOperator::BinaryXor: return LLG_SLANG_OP_BIT_XOR;
    case BinaryOperator::BinaryXnor: return LLG_SLANG_OP_BIT_XNOR;
    case BinaryOperator::Equality: return LLG_SLANG_OP_EQUAL;
    case BinaryOperator::Inequality: return LLG_SLANG_OP_NOT_EQUAL;
    case BinaryOperator::CaseEquality: return LLG_SLANG_OP_CASE_EQUAL;
    case BinaryOperator::CaseInequality: return LLG_SLANG_OP_CASE_NOT_EQUAL;
    case BinaryOperator::WildcardEquality: return LLG_SLANG_OP_WILDCARD_EQUAL;
    case BinaryOperator::WildcardInequality: return LLG_SLANG_OP_WILDCARD_NOT_EQUAL;
    case BinaryOperator::GreaterThan: return LLG_SLANG_OP_GREATER;
    case BinaryOperator::GreaterThanEqual: return LLG_SLANG_OP_GREATER_EQUAL;
    case BinaryOperator::LessThan: return LLG_SLANG_OP_LESS;
    case BinaryOperator::LessThanEqual: return LLG_SLANG_OP_LESS_EQUAL;
    case BinaryOperator::LogicalAnd: return LLG_SLANG_OP_LOGICAL_AND;
    case BinaryOperator::LogicalOr: return LLG_SLANG_OP_LOGICAL_OR;
    case BinaryOperator::LogicalImplication: return LLG_SLANG_OP_LOGICAL_IMPLICATION;
    case BinaryOperator::LogicalEquivalence: return LLG_SLANG_OP_LOGICAL_EQUIVALENCE;
    case BinaryOperator::LogicalShiftLeft: return LLG_SLANG_OP_SHIFT_LEFT;
    case BinaryOperator::LogicalShiftRight: return LLG_SLANG_OP_SHIFT_RIGHT;
    case BinaryOperator::ArithmeticShiftLeft: return LLG_SLANG_OP_ARITH_SHIFT_LEFT;
    case BinaryOperator::ArithmeticShiftRight: return LLG_SLANG_OP_ARITH_SHIFT_RIGHT;
  }
  return LLG_SLANG_OP_NONE;
}

uint32_t semanticTimingKind(TimingControlKind kind) {
  switch (kind) {
    case TimingControlKind::Delay:
    case TimingControlKind::Delay3: return LLG_SLANG_TIMING_DELAY;
    case TimingControlKind::OneStepDelay: return LLG_SLANG_TIMING_ONE_STEP_DELAY;
    case TimingControlKind::SignalEvent: return LLG_SLANG_TIMING_SIGNAL_EVENT;
    case TimingControlKind::EventList:
    case TimingControlKind::BlockEventList: return LLG_SLANG_TIMING_EVENT_LIST;
    case TimingControlKind::ImplicitEvent: return LLG_SLANG_TIMING_IMPLICIT_EVENT;
    case TimingControlKind::RepeatedEvent: return LLG_SLANG_TIMING_REPEATED_EVENT;
    case TimingControlKind::CycleDelay: return LLG_SLANG_TIMING_CYCLE_DELAY;
    default: return LLG_SLANG_SUBKIND_NONE;
  }
}

uint32_t semanticNetKind(NetType::NetKind kind) {
  switch (kind) {
    case NetType::Wire: return LLG_SLANG_NET_WIRE;
    case NetType::WAnd: return LLG_SLANG_NET_WAND;
    case NetType::WOr: return LLG_SLANG_NET_WOR;
    case NetType::Tri: return LLG_SLANG_NET_TRI;
    case NetType::TriAnd: return LLG_SLANG_NET_TRIAND;
    case NetType::TriOr: return LLG_SLANG_NET_TRIOR;
    case NetType::Tri0: return LLG_SLANG_NET_TRI0;
    case NetType::Tri1: return LLG_SLANG_NET_TRI1;
    case NetType::TriReg: return LLG_SLANG_NET_TRIREG;
    case NetType::Supply0: return LLG_SLANG_NET_SUPPLY0;
    case NetType::Supply1: return LLG_SLANG_NET_SUPPLY1;
    case NetType::UWire: return LLG_SLANG_NET_UWIRE;
    case NetType::Interconnect: return LLG_SLANG_NET_INTERCONNECT;
    case NetType::UserDefined: return LLG_SLANG_NET_USER_DEFINED;
    default: return LLG_SLANG_SUBKIND_NONE;
  }
}

uint32_t semanticPrimitiveType(const PrimitiveSymbol& primitive) {
  if (primitive.primitiveKind == PrimitiveSymbol::UserDefined) {
    return primitive.isSequential ? LLG_SLANG_PRIMITIVE_UDP_SEQUENTIAL
                                  : LLG_SLANG_PRIMITIVE_UDP_COMBINATIONAL;
  }
  const std::string_view name = primitive.name;
  if (name == "and") return LLG_SLANG_PRIMITIVE_AND;
  if (name == "nand") return LLG_SLANG_PRIMITIVE_NAND;
  if (name == "nor") return LLG_SLANG_PRIMITIVE_NOR;
  if (name == "or") return LLG_SLANG_PRIMITIVE_OR;
  if (name == "xor") return LLG_SLANG_PRIMITIVE_XOR;
  if (name == "xnor") return LLG_SLANG_PRIMITIVE_XNOR;
  if (name == "buf") return LLG_SLANG_PRIMITIVE_BUF;
  if (name == "not") return LLG_SLANG_PRIMITIVE_NOT;
  if (name == "bufif0") return LLG_SLANG_PRIMITIVE_BUFIF0;
  if (name == "bufif1") return LLG_SLANG_PRIMITIVE_BUFIF1;
  if (name == "notif0") return LLG_SLANG_PRIMITIVE_NOTIF0;
  if (name == "notif1") return LLG_SLANG_PRIMITIVE_NOTIF1;
  if (name == "nmos") return LLG_SLANG_PRIMITIVE_NMOS;
  if (name == "pmos") return LLG_SLANG_PRIMITIVE_PMOS;
  if (name == "cmos") return LLG_SLANG_PRIMITIVE_CMOS;
  if (name == "rnmos") return LLG_SLANG_PRIMITIVE_RNMOS;
  if (name == "rpmos") return LLG_SLANG_PRIMITIVE_RPMOS;
  if (name == "rcmos") return LLG_SLANG_PRIMITIVE_RCMOS;
  if (name == "rtran") return LLG_SLANG_PRIMITIVE_RTRAN;
  if (name == "rtranif0") return LLG_SLANG_PRIMITIVE_RTRANIF0;
  if (name == "rtranif1") return LLG_SLANG_PRIMITIVE_RTRANIF1;
  if (name == "tran") return LLG_SLANG_PRIMITIVE_TRAN;
  if (name == "tranif0") return LLG_SLANG_PRIMITIVE_TRANIF0;
  if (name == "tranif1") return LLG_SLANG_PRIMITIVE_TRANIF1;
  if (name == "pullup") return LLG_SLANG_PRIMITIVE_PULLUP;
  if (name == "pulldown") return LLG_SLANG_PRIMITIVE_PULLDOWN;
  return LLG_SLANG_SUBKIND_NONE;
}

uint32_t semanticTimeUnit(TimeUnit unit) {
  switch (unit) {
    case TimeUnit::Seconds: return LLG_SLANG_TIME_UNIT_SECONDS;
    case TimeUnit::Milliseconds: return LLG_SLANG_TIME_UNIT_MILLISECONDS;
    case TimeUnit::Microseconds: return LLG_SLANG_TIME_UNIT_MICROSECONDS;
    case TimeUnit::Nanoseconds: return LLG_SLANG_TIME_UNIT_NANOSECONDS;
    case TimeUnit::Picoseconds: return LLG_SLANG_TIME_UNIT_PICOSECONDS;
    case TimeUnit::Femtoseconds: return LLG_SLANG_TIME_UNIT_FEMTOSECONDS;
  }
  return LLG_SLANG_TIME_UNIT_NONE;
}

void addTimeScale(LlgSlangSemanticNode& node,
                  const std::optional<TimeScale>& timeScale) {
  if (!timeScale)
    return;
  node.time_unit = semanticTimeUnit(timeScale->base.unit);
  node.time_unit_magnitude = static_cast<uint32_t>(timeScale->base.magnitude);
  node.time_precision_unit = semanticTimeUnit(timeScale->precision.unit);
  node.time_precision_magnitude =
      static_cast<uint32_t>(timeScale->precision.magnitude);
}

uint32_t semanticDriveStrength(const std::optional<DriveStrength>& strength) {
  if (!strength)
    return LLG_SLANG_STRENGTH_UNSPECIFIED;
  switch (*strength) {
    case DriveStrength::Supply: return LLG_SLANG_STRENGTH_SUPPLY;
    case DriveStrength::Strong: return LLG_SLANG_STRENGTH_STRONG;
    case DriveStrength::Pull: return LLG_SLANG_STRENGTH_PULL;
    case DriveStrength::Weak: return LLG_SLANG_STRENGTH_WEAK;
    case DriveStrength::HighZ: return LLG_SLANG_STRENGTH_HIGH_Z;
  }
  return LLG_SLANG_STRENGTH_UNSPECIFIED;
}

void addDriveStrength(LlgSlangSemanticNode& node,
                      const std::pair<std::optional<DriveStrength>,
                                      std::optional<DriveStrength>>& strength) {
  node.strength0 = semanticDriveStrength(strength.first);
  node.strength1 = semanticDriveStrength(strength.second);
}

class SourceInstanceCapture final
    : public syntax::SyntaxVisitor<SourceInstanceCapture> {
public:
  SourceInstanceCapture(Capture& capture, uint64_t definitionId)
      : capture(capture), definitionId(definitionId) {}

  void handle(const syntax::HierarchyInstantiationSyntax& syntaxNode) {
    const std::string_view moduleType = syntaxNode.type.valueText();
    if (moduleType.empty())
      return;

    for (const syntax::HierarchicalInstanceSyntax* instance : syntaxNode.instances) {
      if (!instance || !instance->decl)
        continue;
      const std::string_view name = instance->decl->name.valueText();
      if (name.empty())
        continue;

      const uint64_t id = capture.newSyntheticSemantic();
      auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
      result.parent_id = definitionId;
      result.kind = LLG_SLANG_SEMANTIC_INSTANCE;
      result.subkind = LLG_SLANG_INSTANCE_SINGLE;
      result.flags |= LLG_SLANG_SEMANTIC_UNINSTANTIATED;
      result.name = storeString(capture.output, name);
      result.definition_name = storeString(capture.output, moduleType);
      result.detail = storeString(capture.output, "SourceInstance");
      result.range = capture.span(syntaxNode.type.range());
      capture.semanticChild(definitionId, id);
    }
  }

  // A nested module is a separate definition and must not contribute source
  // instances to its lexical parent.
  void handle(const syntax::ModuleDeclarationSyntax&) {}

private:
  Capture& capture;
  uint64_t definitionId;
};

bool isSyntheticLValue(const Expression& expression) {
  const Expression* current = &expression;
  while (current->kind == ExpressionKind::Conversion)
    current = &current->as<ConversionExpression>().operand();
  return current->kind == ExpressionKind::LValueReference;
}

class SemanticCapture final
    : public ASTVisitor<SemanticCapture, VisitFlags::AllGood | VisitFlags::Bad> {
public:
  explicit SemanticCapture(Capture& capture) : capture(capture) {}

  void bindSourceConnections(
      Compilation& compilation,
      const std::unordered_map<const Symbol*, const InstanceSymbol*>& instances) {
    for (const auto* symbol : sourceConnections) {
      const auto* syntaxNode = symbol->getSyntax();
      const auto* scope = symbol->getParentScope();
      if (!syntaxNode || !scope ||
          syntaxNode->kind != syntax::SyntaxKind::HierarchicalInstance)
        continue;
      const auto* definition =
          compilation.tryGetDefinition(symbol->definitionName, *scope).definition;
      const auto found = instances.find(definition);
      if (found == instances.end())
        continue;
      const auto& body = found->second->body;
      const auto& instance = syntaxNode->as<syntax::HierarchicalInstanceSyntax>();
      for (const auto* connection : instance.connections) {
        if (connection->kind != syntax::SyntaxKind::NamedPortConnection)
          continue;
        const auto& named = connection->as<syntax::NamedPortConnectionSyntax>();
        if (const auto* port = body.findPort(named.name.valueText()))
          capture.lexicalBinding(named.name, captureReferenceTarget(*port),
                                 LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL);
      }
      if (!instance.parent ||
          instance.parent->kind != syntax::SyntaxKind::HierarchyInstantiation)
        continue;
      const auto& hierarchy = instance.parent->as<syntax::HierarchyInstantiationSyntax>();
      const uint64_t definitionId = capture.ensureSemantic(definition);
      capture.lexicalBinding(hierarchy.type, definitionId, LLG_SLANG_LEXICAL_ROLE_REFERENCE);
      const uint64_t id = capture.ensureSemantic(symbol);
      auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
      node.kind = LLG_SLANG_SEMANTIC_INSTANCE;
      node.subkind = LLG_SLANG_INSTANCE_SINGLE;
      node.definition_name = storeString(capture.output, symbol->definitionName);
      node.target_id = definitionId;
      if (!hierarchy.parameters)
        continue;
      for (const auto* assignment : hierarchy.parameters->parameters) {
        if (assignment->kind != syntax::SyntaxKind::NamedParamAssignment)
          continue;
        const auto& named = assignment->as<syntax::NamedParamAssignmentSyntax>();
        for (const auto* parameter : body.getParameters()) {
          if (parameter->symbol.name == named.name.valueText())
            capture.lexicalBinding(named.name, captureReferenceTarget(parameter->symbol),
                                   LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL);
        }
      }
    }
  }

  template<std::derived_from<Symbol> T>
  void handle(const T& symbol) {
    if constexpr (std::same_as<T, InstanceBodySymbol>) {
      if (capture.declarationOnly &&
          !sourceBodies.insert(symbol.getDefinition().getSyntax()).second)
        return;
    }
    if constexpr (std::same_as<T, GenerateBlockSymbol>) {
      if (capture.declarationOnly && symbol.getSyntax() &&
          !sourceGenerateBlocks.insert(symbol.getSyntax()).second)
        return;
    }
    const uint64_t id = capture.ensureSemantic(&symbol);
    attach(id);
    if (!markVisited(id))
      return;
    if constexpr (std::same_as<T, UninstantiatedDefSymbol>) {
      if (capture.declarationOnly)
        sourceConnections.push_back(&symbol);
    }
    capture.sourceIdentity(symbol.getSyntax(), id);
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    result.kind = semanticSymbolKind(symbol.kind);
    result.name = storeString(capture.output, symbol.name);
    result.detail = storeString(capture.output, toString(symbol.kind));
    result.range = symbol.location.valid()
        ? capture.span(symbol.location, symbol.name.size())
        : (symbol.getSyntax() ? capture.span(symbol.getSyntax()->sourceRange())
                              : LlgSlangSourceRange{LLG_SLANG_INVALID_ID, 0, 0});
    if constexpr (std::derived_from<T, ValueSymbol>)
      result.type_id = capture.type(symbol.getType());
    if constexpr (std::same_as<T, GenvarSymbol>) {
      const Scope* parentScope = symbol.getParentScope();
      if (!parentScope)
        throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                            "Slang genvar has no parent scope");
      result.type_id =
          capture.type(parentScope->getCompilation().getIntegerType());
      result.subkind = LLG_SLANG_VARIABLE_GENVAR;
    }
    if constexpr (std::same_as<T, PortSymbol> ||
                  std::same_as<T, MultiPortSymbol>)
      result.type_id = capture.type(symbol.getType());
    if constexpr (std::same_as<T, SubroutineSymbol>)
      result.type_id = capture.type(symbol.getReturnType());
    if constexpr (std::same_as<T, ParameterSymbol>) {
      result.constant_id = capture.constant(symbol.getValue());
      if (symbol.isOverridden())
        result.auxiliary = 1;
      if (symbol.isFromGenvar()) {
        result.flags |= LLG_SLANG_SEMANTIC_IMPLICIT;
        const Scope* blockScope = symbol.getParentScope();
        const Symbol* block = blockScope ? &blockScope->asSymbol() : nullptr;
        const Scope* arrayScope = block ? block->getParentScope() : nullptr;
        const Symbol* array = arrayScope ? &arrayScope->asSymbol() : nullptr;
        if (array && array->kind == SymbolKind::GenerateBlockArray) {
          const auto& generateArray = array->as<GenerateBlockArraySymbol>();
          if (generateArray.loopVariable) {
            const uint64_t loopVariableId =
                capture.ensureSemantic(generateArray.loopVariable);
            capture.sourceIdentity(generateArray.loopVariable->getSyntax(),
                                   loopVariableId);
            capture.lexicalTargetAliases.insert_or_assign(id, loopVariableId);
          }
        }
      }
    }
    if constexpr (std::same_as<T, TypeParameterSymbol>)
      result.type_id = capture.type(symbol.getTypeAlias());
    if constexpr (std::same_as<T, TypeAliasType>)
      result.type_id = capture.type(symbol.targetType.getType());
    if constexpr (std::same_as<T, EnumValueSymbol>)
      result.constant_id = capture.constant(symbol.getValue());
    if constexpr (std::same_as<T, ParameterSymbol> ||
                  std::same_as<T, TypeParameterSymbol>) {
      if (symbol.isLocalParam())
        result.flags |= LLG_SLANG_SEMANTIC_LOCAL;
    }
    if constexpr (std::same_as<T, InstanceSymbol>) {
      result.subkind = LLG_SLANG_INSTANCE_SINGLE;
      if (!symbol.arrayPath.empty()) {
        // arrayPath is canonical, ascending storage order. Preserve each
        // declared logical index before array containers are flattened by Db.
        // This is the same mapping used by Slang's hierarchical path printer.
        SmallVector<ConstantRange, 8> dimensions;
        symbol.getArrayDimensions(dimensions);
        if (dimensions.size() != symbol.arrayPath.size())
          throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                              "instance array path and dimensions disagree");
        std::string indexedName(symbol.getArrayName());
        for (size_t i = 0; i < dimensions.size(); ++i) {
          if (symbol.arrayPath[i] >= dimensions[i].width())
            throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                                "instance array position is outside its dimension");
          const int64_t index = int64_t(dimensions[i].lower()) +
                                int64_t(symbol.arrayPath[i]);
          indexedName += "[" + std::to_string(index) + "]";
        }
        result.name = storeString(capture.output, indexedName);
      }
      result.definition_name = storeString(capture.output,
                                           symbol.getDefinition().name);
      addDefinitionKind(result, symbol.getDefinition().definitionKind);
      if (symbol.isTopLevel())
        result.flags |= LLG_SLANG_SEMANTIC_TOP;
      addTimeScale(result, symbol.getDefinition().timeScale);
    }
    if constexpr (std::same_as<T, InstanceArraySymbol>)
      result.subkind = LLG_SLANG_INSTANCE_ARRAY;
    if constexpr (std::same_as<T, InstanceBodySymbol>)
      result.subkind = LLG_SLANG_INSTANCE_BODY;
    if constexpr (std::same_as<T, GenerateBlockSymbol>) {
      result.subkind = LLG_SLANG_GENERATE_BLOCK;
      std::string externalName = symbol.getExternalName();
      if (symbol.isUninstantiated)
        result.flags |= LLG_SLANG_SEMANTIC_UNINSTANTIATED;
      if (const SVInt* arrayIndex = symbol.getArrayIndex()) {
        externalName += '[' + arrayIndex->toString() + ']';
        result.constant_id = capture.constant(ConstantValue(*arrayIndex));
      }
      result.name = storeString(capture.output, externalName);
    }
    if constexpr (std::same_as<T, GenerateBlockArraySymbol>) {
      result.subkind = LLG_SLANG_GENERATE_BLOCK_ARRAY;
      result.name = storeString(capture.output, symbol.getExternalName());
    }
    if constexpr (std::same_as<T, DefinitionSymbol>) {
      addDefinitionKind(result, symbol.definitionKind);
      addTimeScale(result, symbol.timeScale);
      if (symbol.getInstanceCount() == 0)
        result.flags |= LLG_SLANG_SEMANTIC_UNINSTANTIATED;
      if (symbol.definitionKind == DefinitionKind::Module) {
        const syntax::SyntaxNode* syntaxNode = symbol.getSyntax();
        if (syntaxNode && syntax::ModuleDeclarationSyntax::isKind(syntaxNode->kind)) {
          SourceInstanceCapture sourceInstances(capture, id);
          const auto& declaration = syntaxNode->as<syntax::ModuleDeclarationSyntax>();
          for (const syntax::MemberSyntax* member : declaration.members)
            member->visit(sourceInstances);
        }
      }
    }
    if constexpr (std::same_as<T, PackageSymbol>)
      addTimeScale(result, symbol.timeScale);
    if constexpr (std::same_as<T, CompilationUnitSymbol>)
      addTimeScale(result, symbol.timeScale);
    if constexpr (std::same_as<T, NetSymbol>) {
      result.subkind = semanticNetKind(symbol.netType.netKind);
      if (symbol.isImplicit)
        result.flags |= LLG_SLANG_SEMANTIC_IMPLICIT;
      addDriveStrength(result, symbol.getDriveStrength());
    }
    if constexpr (std::same_as<T, PrimitiveSymbol>) {
      result.subkind = semanticPrimitiveType(symbol);
      result.flags |= LLG_SLANG_SEMANTIC_PRIMITIVE_DECLARATION;
    }
    if constexpr (std::same_as<T, PrimitiveInstanceSymbol>) {
      result.subkind = semanticPrimitiveType(symbol.primitiveType);
      result.definition_name = storeString(capture.output,
                                           symbol.primitiveType.name);
      result.flags |= LLG_SLANG_SEMANTIC_PRIMITIVE_INSTANCE;
      addDriveStrength(result, symbol.getDriveStrength());
    }
    if constexpr (std::same_as<T, PrimitivePortSymbol>) {
      result.flags |= LLG_SLANG_SEMANTIC_PRIMITIVE_PORT;
      addDirection(result, symbol.direction);
    }
    if constexpr (std::same_as<T, PortSymbol>)
      addDirection(result, symbol.direction);
    if constexpr (std::same_as<T, MultiPortSymbol>)
      addDirection(result, symbol.direction);
    if constexpr (std::same_as<T, ModportPortSymbol>)
      addDirection(result, symbol.direction);
    if constexpr (std::same_as<T, FormalArgumentSymbol>) {
      addDirection(result, symbol.direction);
      if (symbol.flags.has(VariableFlags::Const))
        result.auxiliary |= LLG_SLANG_ARGUMENT_CONST_REF;
      if (symbol.flags.has(VariableFlags::RefStatic))
        result.auxiliary |= LLG_SLANG_ARGUMENT_REF_STATIC;
    }
    if constexpr (std::same_as<T, AssertionPortSymbol>) {
      if (const auto* declared = symbol.getDeclaredType())
        result.type_id = capture.type(declared->getType());
      if (symbol.direction)
        addDirection(result, *symbol.direction);
    }
    if constexpr (std::same_as<T, ProceduralBlockSymbol>)
      result.subkind = processKind(symbol.procedureKind);
    if constexpr (std::same_as<T, ContinuousAssignSymbol>)
      addDriveStrength(result, symbol.getDriveStrength());
    if constexpr (std::same_as<T, VariableSymbol>) {
      if (symbol.flags.has(VariableFlags::CompilerGenerated))
        result.flags |= LLG_SLANG_SEMANTIC_IMPLICIT;
      if (isNamedEventType(symbol.getType()))
        result.kind = LLG_SLANG_SEMANTIC_NAMED_EVENT;
      addExplicitVariableLifetime(result, symbol);
    }
    if constexpr (std::same_as<T, ClockingBlockSymbol>) {
      result.subkind = LLG_SLANG_SCOPE_CLOCKING_BLOCK;
      if (symbol.isDefault)
        result.auxiliary |= LLG_SLANG_CLOCKING_BLOCK_DEFAULT;
      if (symbol.isGlobal)
        result.auxiliary |= LLG_SLANG_CLOCKING_BLOCK_GLOBAL;
      const ClockingSkew input = symbol.getDefaultInputSkew();
      const ClockingSkew output = symbol.getDefaultOutputSkew();
      result.auxiliary |= clockingEdgeCode(input.edge)
                          << LLG_SLANG_CLOCKING_INPUT_EDGE_SHIFT;
      result.auxiliary |= clockingEdgeCode(output.edge)
                          << LLG_SLANG_CLOCKING_OUTPUT_EDGE_SHIFT;
    }
    if constexpr (std::same_as<T, ClockVarSymbol>) {
      result.subkind = LLG_SLANG_VARIABLE_CLOCKING;
      addDirection(result, symbol.direction);
      result.auxiliary = clockingEdgeCode(symbol.inputSkew.edge)
                       | (clockingEdgeCode(symbol.outputSkew.edge)
                          << LLG_SLANG_CLOCKING_VAR_OUTPUT_EDGE_SHIFT);
    }
    if constexpr (std::same_as<T, LocalAssertionVarSymbol>) {
      result.subkind = LLG_SLANG_VARIABLE_ASSERTION_LOCAL;
      if (symbol.formalPort && symbol.formalPort->direction)
        addDirection(result, *symbol.formalPort->direction);
    }
    if constexpr (std::derived_from<T, VariableSymbol> &&
                  !std::same_as<T, ClockVarSymbol>) {
      if (result.kind == LLG_SLANG_SEMANTIC_VARIABLE ||
          result.kind == LLG_SLANG_SEMANTIC_NAMED_EVENT) {
        result.auxiliary = symbol.lifetime == VariableLifetime::Automatic
            ? LLG_SLANG_VARIABLE_LIFETIME_AUTOMATIC
            : LLG_SLANG_VARIABLE_LIFETIME_STATIC;
      }
    }
    if constexpr (std::same_as<T, SubroutineSymbol>) {
      addLifetime(result, symbol.defaultLifetime);
      if (symbol.flags.has(MethodFlags::Static))
        result.auxiliary |= LLG_SLANG_SUBROUTINE_STATIC;
      if (symbol.isVirtual())
        result.auxiliary |= LLG_SLANG_SUBROUTINE_VIRTUAL;
      if (symbol.flags.has(MethodFlags::Pure) &&
          !symbol.flags.has(MethodFlags::DPIImport))
        result.auxiliary |= LLG_SLANG_SUBROUTINE_PURE;
      if (symbol.flags.has(MethodFlags::Final))
        result.auxiliary |= LLG_SLANG_SUBROUTINE_FINAL;
      if (symbol.flags.has(MethodFlags::Constructor))
        result.auxiliary |= LLG_SLANG_SUBROUTINE_CONSTRUCTOR;
      if (symbol.flags.has(MethodFlags::DPIImport)) {
        result.auxiliary |= LLG_SLANG_SUBROUTINE_DPI_IMPORT;
        if (symbol.flags.has(MethodFlags::DPIContext))
          result.auxiliary |= LLG_SLANG_SUBROUTINE_DPI_CONTEXT;
        if (symbol.flags.has(MethodFlags::Pure))
          result.auxiliary |= LLG_SLANG_SUBROUTINE_DPI_PURE;

        // Keep the C linkage spelling in the owned semantic record.  Slang
        // has already validated the identifier, but retaining the fallback
        // makes the boundary robust for syntax-recovered declarations.
        std::string_view cName = symbol.name;
        if (const auto* syntaxNode = symbol.getSyntax();
            syntaxNode && syntaxNode->kind == syntax::SyntaxKind::DPIImport) {
          const auto& dpi = syntaxNode->template as<syntax::DPIImportSyntax>();
          if (!dpi.c_identifier.isMissing())
            cName = dpi.c_identifier.valueText();
        }
        result.definition_name = storeString(capture.output, cName);
      }
    }
    if constexpr (std::same_as<T, ClassType>) {
      result.type_id = capture.type(symbol);
      if (symbol.isAbstract)
        result.auxiliary |= LLG_SLANG_CLASS_ABSTRACT;
      if (symbol.isFinal)
        result.auxiliary |= LLG_SLANG_CLASS_FINAL;
      if (symbol.isInterface)
        result.auxiliary |= LLG_SLANG_CLASS_INTERFACE;
      if (const Type* base = symbol.getBaseClass()) {
        const Type& canonical = base->getCanonicalType();
        if (!canonical.isError()) {
          const uint64_t baseId = capture.ensureSemantic(&canonical);
          capture.output.semantic_nodes[static_cast<size_t>(id)].target_id = baseId;
          capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, baseId);
        }
      }
      // Slang synthesizes the base-constructor call for an extends clause and
      // exposes an owned expression for it. Capture that expression explicitly
      // so lowering can preserve argument evaluation and default binding.
      if (const Expression* baseConstructor = symbol.getBaseConstructorCall()) {
        // The generic symbol visitor pushes the current symbol below this
        // block. Push it locally so the synthetic call and its argument nodes
        // remain owned by the class rather than the enclosing module scope.
        parents.push_back(id);
        baseConstructor->visit(*this);
        parents.pop_back();
        capture.semanticRole(id, baseConstructor, LLG_SLANG_EDGE_BASE_CONSTRUCTOR);
      }
    }
    if constexpr (std::same_as<T, SubroutineSymbol>) {
      if (symbol.subroutineKind == SubroutineKind::Task)
        result.flags |= LLG_SLANG_SEMANTIC_TASK;
    }
    if constexpr (std::same_as<T, InstanceSymbol>) {
      const uint64_t definitionId =
          capture.ensureSemantic(&symbol.getDefinition());
      capture.output.semantic_nodes[static_cast<size_t>(id)].target_id =
          definitionId;
      capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, definitionId);
    }
    if constexpr (std::same_as<T, ModportPortSymbol>) {
      if (symbol.internalSymbol) {
        const uint64_t internalId =
            capture.ensureSemantic(symbol.internalSymbol);
        capture.output.semantic_nodes[static_cast<size_t>(id)].target_id =
            internalId;
        capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, internalId);
      }
    }

    parents.push_back(id);
    if constexpr (std::same_as<T, UninstantiatedDefSymbol>) {
      for (const auto* parameter : symbol.paramExpressions)
        parameter->visit(*this);
      for (const auto* connection : symbol.getPortConnections())
        connection->visit(*this);
    }
    if constexpr (std::same_as<T, SubroutineSymbol>) {
      for (const FormalArgumentSymbol* argument : symbol.getArguments())
        handle(*argument);
      if (symbol.returnValVar) {
        handle(*symbol.returnValVar);
        const uint64_t returnId = capture.ensureSemantic(symbol.returnValVar);
        capture.output.semantic_nodes[static_cast<size_t>(returnId)].target_id =
            id;
        capture.semanticEdge(returnId, LLG_SLANG_EDGE_RETURN_OWNER, id);
      }
      // DPI imports are declarations only.  Visiting getBody() would expose
      // native syntax implementation details and can manufacture a phantom
      // body node for a prototype with no body.
      if (!symbol.flags.has(MethodFlags::DPIImport))
        symbol.getBody().visit(*this);
    }
    if constexpr (std::same_as<T, PrimitiveInstanceSymbol>) {
      if (const TimingControl* delay = symbol.getDelay())
        delay->visit(*this);
    }
    if constexpr (std::same_as<T, GenerateBlockArraySymbol>) {
      const GenvarSymbol* sourceGenvar = nullptr;
      for (const Symbol& member : symbol.members()) {
        if (member.kind == SymbolKind::Genvar) {
          sourceGenvar = &member.as<GenvarSymbol>();
          break;
        }
      }
      if (sourceGenvar) {
        handle(*sourceGenvar);
        const uint64_t sourceId = capture.ensureSemantic(sourceGenvar);
        if (symbol.loopVariable) {
          capture.lexicalTargetAliases.insert_or_assign(
              capture.ensureSemantic(symbol.loopVariable), sourceId);
        }
      }
      if (symbol.loopVariable) {
        const uint64_t loopVariableId =
            capture.ensureSemantic(symbol.loopVariable);
        capture.sourceIdentity(symbol.loopVariable->getSyntax(), loopVariableId);
        if (const syntax::SyntaxNode* syntaxNode = symbol.getSyntax();
            syntaxNode && syntaxNode->kind == syntax::SyntaxKind::LoopGenerate) {
          capture.genvarLexicalScope(syntaxNode->sourceRange(),
                                     symbol.loopVariable->name,
                                     loopVariableId);
        }
        for (const GenerateBlockSymbol* entry : symbol.entries) {
          if (capture.declarationOnly)
            break;
          for (const ParameterSymbol& parameter :
               entry->membersOfType<ParameterSymbol>()) {
            if (parameter.isFromGenvar()) {
              capture.lexicalTargetAliases.insert_or_assign(
                  capture.ensureSemantic(&parameter), loopVariableId);
            }
          }
        }
      }
      if (symbol.initialExpression)
        symbol.initialExpression->visit(*this);
      if (symbol.stopExpression)
        symbol.stopExpression->visit(*this);
      if (symbol.iterExpression)
        symbol.iterExpression->visit(*this);
    }
    if constexpr (std::same_as<T, TypeAliasType>) {
      const Type& target = symbol.targetType.getType().getCanonicalType();
      if (target.kind == SymbolKind::EnumType) {
        for (const EnumValueSymbol& value : target.as<EnumType>().values())
          handle(value);
      }
    }
    if constexpr (std::same_as<T, NetAliasSymbol>) {
      uint32_t index = 0;
      for (const Expression* expression : symbol.getNetReferences())
        capture.semanticRole(id, expression, LLG_SLANG_EDGE_ALIAS_NET, index++);
    }
    visitDefault(symbol);
    if constexpr (std::same_as<T, GenerateBlockArraySymbol>) {
      if (symbol.loopVariable) {
        const uint64_t loopVariableId =
            capture.ensureSemantic(symbol.loopVariable);
        for (const GenerateBlockSymbol* entry : symbol.entries) {
          if (capture.declarationOnly)
            break;
          for (const ParameterSymbol& parameter :
               entry->membersOfType<ParameterSymbol>()) {
            if (parameter.isFromGenvar()) {
              capture.lexicalTargetAliases.insert_or_assign(
                  capture.ensureSemantic(&parameter), loopVariableId);
            }
          }
        }
      }
    }
    parents.pop_back();
    captureSourceReferences(symbol);
    if constexpr (std::same_as<T, InstanceBodySymbol> ||
                  std::same_as<T, GenerateBlockSymbol>)
      captureScopeConnections(symbol);
    if (!capture.declarationOnly)
      addSymbolRoles(symbol, id);
    else if constexpr (std::same_as<T, InstanceSymbol>)
      addConnectionLabelBindings(symbol);
  }

  template<std::derived_from<Expression> T>
  void handle(const T& expression) {
    // Compound assignments use an internal LValueReferenceExpression as the
    // left operand of Slang's expanded binary RHS. It is an evaluator
    // placeholder, not an executable source expression; retaining it as an
    // owned `Other` node would make an otherwise supported assignment fail
    // semantic reachability validation.
    if constexpr (std::same_as<T, LValueReferenceExpression>) {
      return;
    }

    if (capture.declarationOnly) {
      if constexpr (std::same_as<T, NamedValueExpression> ||
                    std::same_as<T, HierarchicalValueExpression> ||
                    std::same_as<T, MemberAccessExpression>) {
        if (expression.syntax) {
          if (const Symbol* target = expression.getSymbolReference()) {
            const auto token = expression.syntax->getLastToken();
            if (token.valueText() == target->name)
              capture.lexicalBinding(token, captureReferenceTarget(*target),
                                     LLG_SLANG_LEXICAL_ROLE_REFERENCE);
          }
        }
      }
      if constexpr (std::same_as<T, CallExpression>) {
        if (!expression.isSystemCall() && expression.syntax &&
            expression.syntax->kind == syntax::SyntaxKind::InvocationExpression) {
          const auto* target = std::get<0>(expression.subroutine);
          const auto token = expression.syntax->template as<syntax::InvocationExpressionSyntax>()
                                 .left->getLastToken();
          if (token.valueText() == target->name)
            capture.lexicalBinding(token, captureReferenceTarget(*target),
                                   LLG_SLANG_LEXICAL_ROLE_REFERENCE);
        }
      }
      visitDefault(expression);
      return;
    }
    const uint64_t id = capture.ensureSemantic(&expression);
    attach(id);
    if (!markVisited(id))
      return;
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    result.kind = LLG_SLANG_SEMANTIC_EXPRESSION;
    result.subkind = semanticExpressionKind(expression.kind);
    result.detail = storeString(capture.output, toString(expression.kind));
    result.range = capture.span(expression.sourceRange);
    result.type_id = capture.type(*expression.type);
    if constexpr (std::same_as<T, IntegerLiteral> ||
                  std::same_as<T, UnbasedUnsizedIntegerLiteral>) {
      result.constant_id = capture.constant(ConstantValue(expression.getValue()));
    }
    else if constexpr (std::same_as<T, RealLiteral> ||
                       std::same_as<T, TimeLiteral>) {
      result.constant_id =
          capture.constant(ConstantValue(real_t(expression.getValue())));
    }
    else if constexpr (std::same_as<T, StringLiteral>) {
      result.constant_id = capture.constant(
          ConstantValue(std::string(expression.getValue())));
    }
    else if (const ConstantValue* value = expression.getConstant()) {
      result.constant_id = capture.constant(*value);
    }
    if constexpr (std::same_as<T, AssignmentExpression>) {
      result.operation = expression.op
          ? semanticBinaryOperation(*expression.op)
          : static_cast<uint32_t>(LLG_SLANG_OP_ASSIGN);
      if (expression.isNonBlocking())
        result.flags |= LLG_SLANG_SEMANTIC_NONBLOCKING;
    }
    if constexpr (std::same_as<T, UnaryExpression>)
      result.operation = semanticUnaryOperation(expression.op);
    if constexpr (std::same_as<T, BinaryExpression>)
      result.operation = semanticBinaryOperation(expression.op);
    if constexpr (std::same_as<T, ConditionalExpression>)
      result.operation = LLG_SLANG_OP_CONDITIONAL;
    if constexpr (std::same_as<T, ConcatenationExpression>)
      result.operation = LLG_SLANG_OP_CONCAT;
    if constexpr (std::same_as<T, ReplicationExpression>)
      result.operation = LLG_SLANG_OP_REPLICATE;
    if constexpr (std::same_as<T, StreamingConcatenationExpression>) {
      result.operation = expression.getSliceSize() == 0
          ? LLG_SLANG_OP_STREAM_LEFT
          : LLG_SLANG_OP_STREAM_RIGHT;
      result.auxiliary = expression.getSliceSize();
    }
    if constexpr (std::same_as<T, InsideExpression>)
      result.operation = LLG_SLANG_OP_INSIDE;
    if constexpr (std::derived_from<T, AssignmentPatternExpressionBase>)
      result.operation = LLG_SLANG_OP_ASSIGNMENT_PATTERN;
    if constexpr (std::same_as<T, ReplicatedAssignmentPatternExpression>)
      result.operation = LLG_SLANG_OP_MULTI_ASSIGNMENT_PATTERN;
    if constexpr (std::same_as<T, MinTypMaxExpression>)
      result.operation = LLG_SLANG_OP_MIN_TYP_MAX;
    if constexpr (std::same_as<T, ValueRangeExpression>) {
      if (expression.rangeKind == ValueRangeKind::Simple)
        result.operation = LLG_SLANG_OP_LIST;
      else {
        result.kind = LLG_SLANG_SEMANTIC_UNSUPPORTED;
        result.subkind = LLG_SLANG_SUBKIND_NONE;
      }
    }
    if constexpr (std::same_as<T, TimeLiteral>) {
      addTimeScale(result, expression.getScale());
      // Literal names carry the exact post-preprocessing token spelling.
      // Source ranges point to macro invocation sites and cannot reconstruct
      // conditional definitions, include-file macros, or token concatenation.
      if (expression.syntax)
        result.name = storeString(capture.output,
                                 expression.syntax->getFirstToken().rawText());
    }
    if constexpr (std::same_as<T, ConversionExpression>) {
      if (expression.isImplicit())
        result.flags |= LLG_SLANG_SEMANTIC_IMPLICIT_CONVERSION;
      if (expression.conversionKind == ConversionKind::Propagated)
        result.flags |= LLG_SLANG_SEMANTIC_PROPAGATED_CONVERSION;
    }
    if constexpr (std::same_as<T, RangeSelectExpression>) {
      if (expression.getSelectionKind() == RangeSelectionKind::IndexedUp)
        result.flags |= LLG_SLANG_SEMANTIC_INDEXED_UP;
      else if (expression.getSelectionKind() == RangeSelectionKind::IndexedDown)
        result.flags |= LLG_SLANG_SEMANTIC_INDEXED_DOWN;
    }
    if constexpr (std::same_as<T, MemberAccessExpression>)
      result.name = storeString(capture.output, expression.member.name);
    if constexpr (std::same_as<T, CallExpression>) {
      result.name = storeString(capture.output, expression.getSubroutineName());
      if (expression.isSystemCall()) {
        const auto& info =
            std::get<CallExpression::SystemCallInfo>(expression.subroutine);
        const auto [iteratorExpression, iteratorVariable] =
            info.getIteratorInfo();
        if (iteratorExpression || iteratorVariable)
          result.flags |= LLG_SLANG_SEMANTIC_METHOD_WITH_CLAUSE;
        // Keep the iterator declaration identity on the owned call record.
        // The iterator expression is visited as a structural child below,
        // but its implicit `item` binding is not part of the call arguments.
        // Capturing it here lets downstream consumers bind the callback
        // without guessing from source names or child ordering.
        if (iteratorVariable)
          result.target_id = capture.ensureSemantic(iteratorVariable);
      }
      if (isSystemMethodCall(expression))
        result.kind = LLG_SLANG_SEMANTIC_METHOD_CALL;
      else if (expression.isSystemCall())
        result.kind = LLG_SLANG_SEMANTIC_SYSTEM_CALL;
      else if (expression.thisClass())
        result.kind = LLG_SLANG_SEMANTIC_METHOD_CALL;
      else
        result.kind = LLG_SLANG_SEMANTIC_FUNCTION_CALL;
      if (!expression.isSystemCall()) {
        const SubroutineSymbol* callee = std::get<0>(expression.subroutine);
        if (callee && callee->subroutineKind == SubroutineKind::Task)
          result.flags |= LLG_SLANG_SEMANTIC_TASK;
      }
    }
    if constexpr (std::same_as<T, AssertionInstanceExpression>) {
      result.name = storeString(capture.output, expression.symbol.name);
      const uint64_t target = capture.ensureSemantic(&expression.symbol);
      result.target_id = target;
      capture.semanticEdge(id, LLG_SLANG_EDGE_CALLEE, target);
    }
    if (const Symbol* target = expression.getSymbolReference()) {
      const uint64_t targetId = capture.ensureSemantic(target);
      auto& targetNode =
          capture.output.semantic_nodes[static_cast<size_t>(targetId)];
      if (targetNode.name.len == 0) {
        targetNode.kind = semanticSymbolKind(target->kind);
        if (target->kind == SymbolKind::LocalAssertionVar)
          targetNode.subkind = LLG_SLANG_VARIABLE_ASSERTION_LOCAL;
        targetNode.name = storeString(capture.output, target->name);
        targetNode.detail = storeString(capture.output, toString(target->kind));
        targetNode.range = target->location.valid()
            ? capture.span(target->location, target->name.size())
            : LlgSlangSourceRange{LLG_SLANG_INVALID_ID, 0, 0};
      }
      if (targetNode.type_id == LLG_SLANG_INVALID_ID &&
          ValueSymbol::isKind(target->kind)) {
        targetNode.type_id = capture.type(target->as<ValueSymbol>().getType());
      }
      capture.output.semantic_nodes[static_cast<size_t>(id)].target_id = targetId;
      capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, targetId);
    }
    if constexpr (std::same_as<T, HierarchicalValueExpression>) {
      if (expression.syntax) {
        const parsing::Token firstToken = expression.syntax->getFirstToken();
        for (const auto& element : expression.ref.path) {
          if (element.symbol->name == firstToken.valueText()) {
            capture.lexicalBinding(
                firstToken, capture.ensureSemantic(element.symbol),
                LLG_SLANG_LEXICAL_ROLE_REFERENCE);
            break;
          }
        }
      }
    }
    parents.push_back(id);
    if constexpr (std::same_as<T, AssignmentExpression>) {
      // A compound assignment's right expression is Slang's expanded binary
      // operation. Visit its real RHS directly so the evaluator-only lvalue
      // placeholder in the expansion never enters the owned graph.
      if (expression.op) {
        if (expression.timingControl)
          expression.timingControl->visit(*this);
        expression.left().visit(*this);
        if (const Expression* rhs = compoundAssignmentSourceRhs(expression))
          rhs->visit(*this);
        else
          visitDefault(expression);
      }
      else {
        visitDefault(expression);
      }
    }
    else if constexpr (std::same_as<T, AssertionInstanceExpression>) {
      // Assertion-instance arguments are not traversed by AssertionExpr's
      // ordinary expression visitor. Visit each actual explicitly so the
      // owned semantic graph retains its value/sequence/event binding.
      expression.body.visit(*this);
      for (const auto& [formal, actual] : expression.arguments) {
        (void)formal;
        std::visit([this](auto* value) {
          if (value)
            value->visit(*this);
        }, actual);
      }
    }
    else if constexpr (std::same_as<T, BinaryExpression>) {
      // Slang expands a compound assignment into a binary RHS whose left
      // operand is an evaluator-only lvalue placeholder. Do not capture that
      // synthetic subtree as an executable child; the assignment node owns
      // the real target and the binary node only supplies the computed value.
      if (isSyntheticLValue(expression.left()))
        expression.right().visit(*this);
      else
        visitDefault(expression);
    }
    else {
      visitDefault(expression);
    }
    parents.pop_back();
    addExpressionRoles(expression, id);
    capture.replaceChildRoles(id, LLG_SLANG_EDGE_OPERAND);
  }

  template<std::derived_from<Statement> T>
  void handle(const T& statement) {
    if (capture.declarationOnly) {
      if constexpr (std::same_as<T, VariableDeclStatement>)
        handle(statement.symbol);
      if constexpr (std::same_as<T, ForeachLoopStatement>) {
        for (const auto& dimension : statement.loopDims) {
          if (dimension.loopVar)
            handle(*dimension.loopVar);
        }
      }
      visitDefault(statement);
      return;
    }
    const uint64_t id = capture.ensureSemantic(&statement);
    attach(id);
    if (!markVisited(id))
      return;
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    result.kind = LLG_SLANG_SEMANTIC_STATEMENT;
    result.subkind = semanticStatementKind(statement.kind);
    result.detail = storeString(capture.output, toString(statement.kind));
    result.range = capture.span(statement.sourceRange);
    if (statement.bad())
      result.flags |= LLG_SLANG_SEMANTIC_BAD;
    if constexpr (std::same_as<T, CaseStatement>) {
      result.auxiliary = semanticUniquePriorityCheck(statement.check);
      switch (statement.condition) {
        case CaseStatementCondition::WildcardXOrZ:
          result.flags |= LLG_SLANG_SEMANTIC_CASE_X_OR_Z;
          break;
        case CaseStatementCondition::WildcardJustZ:
          result.flags |= LLG_SLANG_SEMANTIC_CASE_Z;
          break;
        case CaseStatementCondition::Inside:
          result.flags |= LLG_SLANG_SEMANTIC_CASE_INSIDE;
          break;
        default: break;
      }
    }
    if constexpr (std::same_as<T, ConditionalStatement>)
      result.auxiliary = semanticUniquePriorityCheck(statement.check);
    if constexpr (std::same_as<T, ImmediateAssertionStatement>) {
      switch (statement.assertionKind) {
        case AssertionKind::Assert:
          result.subkind = LLG_SLANG_STMT_IMMEDIATE_ASSERT;
          break;
        case AssertionKind::Assume:
          result.subkind = LLG_SLANG_STMT_IMMEDIATE_ASSUME;
          break;
        case AssertionKind::CoverProperty:
        case AssertionKind::CoverSequence:
          result.subkind = LLG_SLANG_STMT_IMMEDIATE_COVER;
          break;
        default:
          // Keep assertion forms outside the Q03 subset explicit. The safe
          // facade will retain the node but downstream lowering rejects it.
          result.subkind = LLG_SLANG_SUBKIND_NONE;
          result.flags |= LLG_SLANG_SEMANTIC_BAD;
          break;
      }
      if (statement.syntax && statement.syntax->label)
        result.name = storeString(capture.output, statement.syntax->label->name.valueText());
      if (statement.isDeferred)
        result.auxiliary |= LLG_SLANG_ASSERTION_DEFERRED;
      if (statement.isFinal)
        result.auxiliary |= LLG_SLANG_ASSERTION_FINAL;
    }
    if constexpr (std::same_as<T, ConcurrentAssertionStatement>) {
      switch (statement.assertionKind) {
        case AssertionKind::Assert:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_ASSERT;
          break;
        case AssertionKind::Assume:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_ASSUME;
          break;
        case AssertionKind::CoverProperty:
        case AssertionKind::CoverSequence:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_COVER;
          break;
        case AssertionKind::Expect:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_EXPECT;
          break;
        default:
          result.subkind = LLG_SLANG_SUBKIND_NONE;
          result.flags |= LLG_SLANG_SEMANTIC_BAD;
          break;
      }
      if (statement.syntax && statement.syntax->label)
        result.name = storeString(capture.output, statement.syntax->label->name.valueText());
    }
    if constexpr (std::same_as<T, ProceduralAssignStatement>) {
      result.subkind = statement.isForce ? LLG_SLANG_STMT_FORCE
                                         : LLG_SLANG_STMT_PROCEDURAL_ASSIGN;
    }
    if constexpr (std::same_as<T, ProceduralDeassignStatement>) {
      result.subkind = statement.isRelease
          ? LLG_SLANG_STMT_RELEASE
          : LLG_SLANG_STMT_PROCEDURAL_DEASSIGN;
    }
    if constexpr (std::same_as<T, BlockStatement>) {
      switch (statement.blockKind) {
        case StatementBlockKind::Sequential:
          result.subkind = LLG_SLANG_STMT_BLOCK;
          break;
        case StatementBlockKind::JoinAll:
          result.subkind = LLG_SLANG_STMT_FORK_JOIN_ALL;
          break;
        case StatementBlockKind::JoinAny:
          result.subkind = LLG_SLANG_STMT_FORK_JOIN_ANY;
          break;
        case StatementBlockKind::JoinNone:
          result.subkind = LLG_SLANG_STMT_FORK_JOIN_NONE;
          break;
      }
      if (statement.blockSymbol) {
        const uint64_t target = capture.ensureSemantic(statement.blockSymbol);
        capture.output.semantic_nodes[static_cast<size_t>(id)].target_id = target;
      }
    }
    parents.push_back(id);
    if constexpr (std::same_as<T, VariableDeclStatement>)
      handle(statement.symbol);
    if constexpr (std::same_as<T, ForeachLoopStatement>) {
      for (const auto& dimension : statement.loopDims) {
        if (dimension.loopVar)
          handle(*dimension.loopVar);
      }
    }
    visitDefault(statement);
    parents.pop_back();
    addStatementRoles(statement, id);
  }

  template<typename T>
    requires(std::derived_from<T, TimingControl> ||
             std::derived_from<T, Constraint> ||
             std::derived_from<T, AssertionExpr> ||
             std::derived_from<T, Pattern>)
  void handle(const T& node) {
    if (capture.declarationOnly) {
      visitDefault(node);
      return;
    }
    const uint64_t id = capture.ensureSemantic(&node);
    attach(id);
    if (!markVisited(id))
      return;
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    if constexpr (std::derived_from<T, TimingControl>) {
      result.kind = LLG_SLANG_SEMANTIC_TIMING_CONTROL;
      result.subkind = semanticTimingKind(node.kind);
      if constexpr (std::same_as<T, SignalEventControl>) {
        switch (node.edge) {
          case EdgeKind::PosEdge: result.flags |= LLG_SLANG_SEMANTIC_POSEDGE; break;
          case EdgeKind::NegEdge: result.flags |= LLG_SLANG_SEMANTIC_NEGEDGE; break;
          case EdgeKind::BothEdges:
            result.flags |= LLG_SLANG_SEMANTIC_BOTH_EDGES;
            break;
          default: break;
        }
      }
    }
    else if constexpr (std::derived_from<T, AssertionExpr>) {
      result.kind = LLG_SLANG_SEMANTIC_ASSERTION_EXPR;
      result.subkind = semanticAssertionExprKind(node.kind);
      auto setRange = [&](const SequenceRange& range, uint32_t repetitionKind) {
        result.assertion_range_min = range.min;
        result.assertion_range_max = range.max.value_or(LLG_SLANG_ASSERTION_RANGE_UNBOUNDED);
        result.assertion_repetition_kind = repetitionKind;
      };
      if constexpr (std::same_as<T, SimpleAssertionExpr>) {
        if (node.repetition) {
          result.auxiliary |= LLG_SLANG_ASSERTION_REPETITION;
          uint32_t kind = LLG_SLANG_ASSERTION_REPEAT_CONSECUTIVE;
          if (node.repetition->kind == SequenceRepetition::Nonconsecutive)
            kind = LLG_SLANG_ASSERTION_REPEAT_NONCONSECUTIVE;
          else if (node.repetition->kind == SequenceRepetition::GoTo)
            kind = LLG_SLANG_ASSERTION_REPEAT_GOTO;
          setRange(node.repetition->range, kind);
        }
      }
      else if constexpr (std::same_as<T, SequenceWithMatchExpr>) {
        if (node.repetition) {
          result.auxiliary |= LLG_SLANG_ASSERTION_REPETITION;
          uint32_t kind = LLG_SLANG_ASSERTION_REPEAT_CONSECUTIVE;
          if (node.repetition->kind == SequenceRepetition::Nonconsecutive)
            kind = LLG_SLANG_ASSERTION_REPEAT_NONCONSECUTIVE;
          else if (node.repetition->kind == SequenceRepetition::GoTo)
            kind = LLG_SLANG_ASSERTION_REPEAT_GOTO;
          setRange(node.repetition->range, kind);
        }
      }
      else if constexpr (std::same_as<T, UnaryAssertionExpr>) {
        result.operation = semanticUnaryAssertionOperation(node.op);
        if (node.range) {
          result.auxiliary |= LLG_SLANG_ASSERTION_RANGE;
          setRange(*node.range, 0);
        }
      }
      else if constexpr (std::same_as<T, BinaryAssertionExpr>) {
        result.operation = semanticBinaryAssertionOperation(node.op);
      }
      else if constexpr (std::same_as<T, StrongWeakAssertionExpr>) {
        if (node.strength == StrongWeakAssertionExpr::Strong)
          result.auxiliary |= LLG_SLANG_ASSERTION_STRONG;
      }
      else if constexpr (std::same_as<T, AbortAssertionExpr>) {
        if (node.action == AbortAssertionExpr::Reject)
          result.auxiliary |= LLG_SLANG_ASSERTION_ABORT_REJECT;
        if (node.isSync)
          result.auxiliary |= LLG_SLANG_ASSERTION_ABORT_SYNC;
      }
    }
    else {
      result.kind = LLG_SLANG_SEMANTIC_UNSUPPORTED;
    }
    result.detail = storeString(capture.output, toString(node.kind));
    if constexpr (requires { node.sourceRange; })
      result.range = capture.span(node.sourceRange);
    parents.push_back(id);
    visitDefault(node);
    parents.pop_back();
    addAuxiliaryRoles(node, id);
  }

private:
  void captureNames(const syntax::SyntaxNode& syntaxNode, const ASTContext& context) {
    auto visitor = syntax::makeSyntaxVisitor(
        [&](auto&, const syntax::NameSyntax& name) {
          LookupResult result;
          Lookup::name(name, context, LookupFlags::None, result);
          if (!result.found || result.hasError() || !result.selectors.empty())
            return;
          const auto token = name.getLastToken();
          if (token.valueText() == result.found->name)
            capture.lexicalBinding(token, captureReferenceTarget(*result.found),
                                   LLG_SLANG_LEXICAL_ROLE_REFERENCE);
        });
    syntaxNode.visit(visitor);
  }

  void captureScopeConnections(const Scope& scope) {
    const auto* syntaxNode = scope.asSymbol().getSyntax();
    if (!syntaxNode)
      return;
    const ASTContext context(scope, LookupLocation::max);
    auto visitor = syntax::makeSyntaxVisitor(
        [&](auto&, const syntax::HierarchyInstantiationSyntax& instance) {
          if (instance.parameters)
            captureNames(*instance.parameters, context);
          for (const auto* child : instance.instances) {
            for (const auto* connection : child->connections)
              captureNames(*connection, context);
          }
        },
        [](auto& self, const syntax::GenerateRegionSyntax& region) {
          self.visitDefault(region);
        },
        [](auto&, const syntax::MemberSyntax&) {});
    // Nested generate / module scopes are captured with their own symbol tables.
    if (syntaxNode->kind == syntax::SyntaxKind::HierarchyInstantiation)
      syntaxNode->visit(visitor);
    else
      visitor.visitDefault(*syntaxNode);
  }

  void captureSourceReferences(const Symbol& symbol) {
    const auto* declaredType = symbol.getDeclaredType();
    const auto* scope = symbol.getParentScope();
    if (!scope || (!declaredType &&
        !(capture.declarationOnly && symbol.kind == SymbolKind::ContinuousAssign)))
      return;
    // Slang's AST visitor visits initializers, but not names in type dimensions.
    const ASTContext context(*scope, LookupLocation::before(symbol));
    if (declaredType) {
      if (const auto* typeSyntax = declaredType->getTypeSyntax())
        captureNames(*typeSyntax, context);
      if (const auto* dimensions = declaredType->getDimensionSyntax()) {
        for (const auto* dimension : *dimensions)
          captureNames(*dimension, context);
      }
      if (capture.declarationOnly) {
        if (const auto* initializer = declaredType->getInitializerSyntax())
          captureNames(*initializer, context);
      }
    }
    // Invalid source instances can have an error-typed assignment with no AST operands.
    if (capture.declarationOnly && symbol.kind == SymbolKind::ContinuousAssign) {
      if (const auto* syntaxNode = symbol.getSyntax())
        captureNames(*syntaxNode, context);
    }
  }

  Capture& capture;
  std::vector<uint64_t> parents;
  std::vector<bool> visited;
  std::unordered_set<const syntax::SyntaxNode*> sourceBodies;
  std::unordered_set<const syntax::SyntaxNode*> sourceGenerateBlocks;
  std::vector<const UninstantiatedDefSymbol*> sourceConnections;

  uint64_t captureReferenceTarget(const Symbol& symbol) {
    const uint64_t id = capture.ensureSemantic(&symbol);
    auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
    if (node.name.len == 0) {
      node.kind = semanticSymbolKind(symbol.kind);
      if (symbol.kind == SymbolKind::LocalAssertionVar)
        node.subkind = LLG_SLANG_VARIABLE_ASSERTION_LOCAL;
      node.name = storeString(capture.output, symbol.name);
      node.detail = storeString(capture.output, toString(symbol.kind));
      node.range = capture.span(symbol.location, symbol.name.size());
      capture.sourceIdentity(symbol.getSyntax(), id);
    }
    if (ValueSymbol::isKind(symbol.kind))
      node.type_id = capture.type(symbol.as<ValueSymbol>().getType());
    if (symbol.kind == SymbolKind::Port) {
      const auto& port = symbol.as<PortSymbol>();
      node.type_id = capture.type(port.getType());
      addDirection(node, port.direction);
    }
    return id;
  }

  bool markVisited(uint64_t id) {
    if (visited.size() <= id)
      visited.resize(static_cast<size_t>(id + 1));
    if (visited[static_cast<size_t>(id)])
      return false;
    visited[static_cast<size_t>(id)] = true;
    return true;
  }

  void attach(uint64_t id) {
    if (parents.empty())
      return;
    auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
    if (node.parent_id == LLG_SLANG_INVALID_ID)
      node.parent_id = parents.back();
    uint32_t childIndex = 0;
    for (const auto& edge :
         capture.pendingEdges[static_cast<size_t>(parents.back())]) {
      if (edge.role == LLG_SLANG_EDGE_CHILD && edge.target_id == id)
        return;
      if (edge.role == LLG_SLANG_EDGE_CHILD)
        childIndex = std::max(childIndex, edge.index + 1);
    }
    capture.semanticEdge(parents.back(), LLG_SLANG_EDGE_CHILD, id, childIndex);
  }

  static uint32_t processKind(ProceduralBlockKind kind) {
    switch (kind) {
      case ProceduralBlockKind::Initial: return LLG_SLANG_PROCESS_INITIAL;
      case ProceduralBlockKind::Final: return LLG_SLANG_PROCESS_FINAL;
      case ProceduralBlockKind::Always: return LLG_SLANG_PROCESS_ALWAYS;
      case ProceduralBlockKind::AlwaysComb: return LLG_SLANG_PROCESS_ALWAYS_COMB;
      case ProceduralBlockKind::AlwaysLatch: return LLG_SLANG_PROCESS_ALWAYS_LATCH;
      case ProceduralBlockKind::AlwaysFF: return LLG_SLANG_PROCESS_ALWAYS_FF;
    }
    return LLG_SLANG_SUBKIND_NONE;
  }

  static void addDirection(LlgSlangSemanticNode& node,
                           ArgumentDirection direction) {
    switch (direction) {
      case ArgumentDirection::In: node.flags |= LLG_SLANG_SEMANTIC_INPUT; break;
      case ArgumentDirection::Out: node.flags |= LLG_SLANG_SEMANTIC_OUTPUT; break;
      case ArgumentDirection::InOut: node.flags |= LLG_SLANG_SEMANTIC_INOUT; break;
      case ArgumentDirection::Ref: node.flags |= LLG_SLANG_SEMANTIC_REF; break;
    }
  }

  static void addDirection(LlgSlangSemanticNode& node,
                           PrimitivePortDirection direction) {
    switch (direction) {
      case PrimitivePortDirection::In:
        node.flags |= LLG_SLANG_SEMANTIC_INPUT;
        break;
      case PrimitivePortDirection::Out:
      case PrimitivePortDirection::OutReg:
        node.flags |= LLG_SLANG_SEMANTIC_OUTPUT;
        break;
      case PrimitivePortDirection::InOut:
        node.flags |= LLG_SLANG_SEMANTIC_INOUT;
        break;
    }
  }

  static void addLifetime(LlgSlangSemanticNode& node,
                          VariableLifetime lifetime) {
    if (lifetime == VariableLifetime::Automatic)
      node.flags |= LLG_SLANG_SEMANTIC_AUTOMATIC;
    else
      node.flags |= LLG_SLANG_SEMANTIC_STATIC;
  }

  static void addExplicitVariableLifetime(LlgSlangSemanticNode& node,
                                          const VariableSymbol& symbol) {
    const syntax::SyntaxNode* syntaxNode = symbol.getSyntax();
    if (!syntaxNode)
      return;
    if (syntaxNode->kind == syntax::SyntaxKind::Declarator)
      syntaxNode = syntaxNode->parent;
    if (!syntaxNode || syntaxNode->kind != syntax::SyntaxKind::DataDeclaration)
      return;

    const auto& declaration =
        syntaxNode->as<syntax::DataDeclarationSyntax>();
    for (parsing::Token modifier : declaration.modifiers) {
      if (modifier.kind == parsing::TokenKind::AutomaticKeyword) {
        node.flags |= LLG_SLANG_SEMANTIC_AUTOMATIC;
        return;
      }
      if (modifier.kind == parsing::TokenKind::StaticKeyword) {
        node.flags |= LLG_SLANG_SEMANTIC_STATIC;
        return;
      }
    }
  }

  static void addDefinitionKind(LlgSlangSemanticNode& node,
                                DefinitionKind kind) {
    switch (kind) {
      case DefinitionKind::Module: node.flags |= LLG_SLANG_SEMANTIC_MODULE; break;
      case DefinitionKind::Interface: node.flags |= LLG_SLANG_SEMANTIC_INTERFACE; break;
      case DefinitionKind::Program: node.flags |= LLG_SLANG_SEMANTIC_PROGRAM; break;
    }
  }

  template<std::derived_from<Symbol> T>
  void addSymbolRoles(const T& symbol, uint64_t id) {
    if constexpr (std::derived_from<T, ValueSymbol>) {
      if (const Expression* initializer = symbol.getInitializer())
        capture.semanticRole(id, initializer, LLG_SLANG_EDGE_INITIALIZER);
    }
    if constexpr (std::same_as<T, ClockingBlockSymbol>) {
      capture.semanticRole(id, &symbol.getEvent(), LLG_SLANG_EDGE_EVENT);
      const ClockingSkew input = symbol.getDefaultInputSkew();
      if (input.delay) {
        input.delay->visit(*this);
        capture.semanticRole(id, input.delay, LLG_SLANG_EDGE_DELAY, 0);
      }
      const ClockingSkew output = symbol.getDefaultOutputSkew();
      if (output.delay) {
        output.delay->visit(*this);
        capture.semanticRole(id, output.delay, LLG_SLANG_EDGE_DELAY, 1);
      }
    }
    if constexpr (std::same_as<T, ClockVarSymbol>) {
      if (symbol.inputSkew.delay) {
        symbol.inputSkew.delay->visit(*this);
        capture.semanticRole(id, symbol.inputSkew.delay, LLG_SLANG_EDGE_DELAY, 0);
      }
      if (symbol.outputSkew.delay) {
        symbol.outputSkew.delay->visit(*this);
        capture.semanticRole(id, symbol.outputSkew.delay, LLG_SLANG_EDGE_DELAY, 1);
      }
    }
    if constexpr (std::same_as<T, PortSymbol>) {
      if (const Expression* internal = symbol.getInternalExpr()) {
        capture.semanticRole(id, internal, LLG_SLANG_EDGE_LOW_CONNECTION);
      }
      else if (symbol.internalSymbol) {
        capture.semanticRole(id, symbol.internalSymbol,
                             LLG_SLANG_EDGE_LOW_CONNECTION);
      }
      if (const Expression* initializer = symbol.getInitializer())
        capture.semanticRole(id, initializer, LLG_SLANG_EDGE_INITIALIZER);
    }
    if constexpr (std::same_as<T, SubroutineSymbol>) {
      capture.semanticRole(id, &symbol.getBody(), LLG_SLANG_EDGE_BODY);
    }
    if constexpr (std::same_as<T, NetSymbol>) {
      if (const Expression* initializer = symbol.getInitializer()) {
        const uint64_t ownerId =
            capture.output.semantic_nodes[static_cast<size_t>(id)].parent_id;
        if (ownerId != LLG_SLANG_INVALID_ID) {
          const uint64_t assignId = capture.newSyntheticSemantic();
          const uint64_t expressionId = capture.newSyntheticSemantic();
          const uint64_t referenceId = capture.newSyntheticSemantic();
          const uint64_t initializerId = capture.ensureSemantic(initializer);
          const uint64_t typeId = capture.type(symbol.getType());
          const LlgSlangSourceRange range =
              capture.span(symbol.location, symbol.name.size());

          auto& assign = capture.output.semantic_nodes[
              static_cast<size_t>(assignId)];
          assign.parent_id = ownerId;
          assign.kind = LLG_SLANG_SEMANTIC_CONTINUOUS_ASSIGN;
          assign.subkind = LLG_SLANG_CONTINUOUS_ASSIGN_NET_DECL;
          assign.detail = storeString(capture.output, "NetDeclarationAssign");
          assign.range = range;
          addDriveStrength(assign, symbol.getDriveStrength());

          auto& expression = capture.output.semantic_nodes[
              static_cast<size_t>(expressionId)];
          expression.parent_id = assignId;
          expression.kind = LLG_SLANG_SEMANTIC_EXPRESSION;
          expression.subkind = LLG_SLANG_EXPR_ASSIGNMENT;
          expression.operation = LLG_SLANG_OP_ASSIGN;
          expression.detail = storeString(capture.output, "Assignment");
          expression.range = range;
          expression.type_id = typeId;

          auto& reference = capture.output.semantic_nodes[
              static_cast<size_t>(referenceId)];
          reference.parent_id = expressionId;
          reference.kind = LLG_SLANG_SEMANTIC_EXPRESSION;
          reference.subkind = LLG_SLANG_EXPR_REFERENCE;
          reference.name = storeString(capture.output, symbol.name);
          reference.detail = storeString(capture.output, "NamedValue");
          reference.range = range;
          reference.type_id = typeId;
          reference.target_id = id;

          capture.semanticChild(ownerId, assignId);
          capture.semanticEdge(assignId, LLG_SLANG_EDGE_BODY, expressionId);
          capture.semanticEdge(expressionId, LLG_SLANG_EDGE_LHS, referenceId);
          capture.semanticEdge(expressionId, LLG_SLANG_EDGE_RHS, initializerId);
          capture.semanticEdge(referenceId, LLG_SLANG_EDGE_REFERENCE, id);
          if (const TimingControl* delay = symbol.getDelay())
            capture.semanticRole(assignId, delay, LLG_SLANG_EDGE_DELAY);
        }
      }
      if (const TimingControl* delay = symbol.getDelay())
        capture.semanticRole(id, delay, LLG_SLANG_EDGE_DELAY);
    }
    if constexpr (std::same_as<T, FormalArgumentSymbol>) {
      if (const Expression* value = symbol.getDefaultValue())
        capture.semanticRole(id, value, LLG_SLANG_EDGE_DEFAULT_VALUE);
    }
    if constexpr (std::same_as<T, ModportPortSymbol>) {
      if (const Expression* connection = symbol.getConnectionExpr())
        capture.semanticRole(id, connection, LLG_SLANG_EDGE_LOW_CONNECTION);
    }
    if constexpr (std::same_as<T, InterfacePortSymbol>) {
      const auto [connection, expression] = symbol.getConnectionAndExpr();
      if (connection.first) {
        const uint64_t connectionId = capture.newSyntheticSemantic();
        const uint64_t targetId = capture.ensureSemantic(connection.first);
        auto& node =
            capture.output.semantic_nodes[static_cast<size_t>(connectionId)];
        node.parent_id = id;
        node.kind = LLG_SLANG_SEMANTIC_INTERFACE_CONNECTION;
        node.name = storeString(capture.output,
                                connection.second ? connection.second->name : "");
        node.detail = storeString(capture.output, "InterfaceConnection");
        node.range = capture.span(symbol.location, symbol.name.size());
        node.target_id = targetId;
        capture.semanticEdge(id, LLG_SLANG_EDGE_CHILD, connectionId);
        capture.semanticEdge(connectionId, LLG_SLANG_EDGE_REFERENCE,
                             targetId);
        if (expression)
          capture.semanticRole(id, expression, LLG_SLANG_EDGE_HIGH_CONNECTION);
      }
    }
    if constexpr (std::same_as<T, InstanceSymbol>) {
      uint32_t index = 0;
      for (const PortConnection* connection : symbol.getPortConnections()) {
        capture.semanticRole(id, &connection->port, LLG_SLANG_EDGE_DECLARATION,
                             index);
        if (const Expression* actual = connection->getExpression()) {
          capture.semanticRole(id, actual, LLG_SLANG_EDGE_ACTUAL, index);
          const uint64_t portId = capture.ensureSemantic(&connection->port);
          capture.output.semantic_nodes[static_cast<size_t>(portId)].flags |=
              LLG_SLANG_SEMANTIC_PORT_CONNECTION_PRESENT;
          capture.semanticRole(portId, actual, LLG_SLANG_EDGE_HIGH_CONNECTION);
        }
        index++;
      }
      addConnectionLabelBindings(symbol);
    }
    if constexpr (std::same_as<T, ProceduralBlockSymbol>)
      capture.replaceChildRoles(id, LLG_SLANG_EDGE_BODY);
    if constexpr (std::same_as<T, GenerateBlockSymbol>) {
      if (const Expression* condition = symbol.getConditionExpression())
        capture.semanticRole(id, condition, LLG_SLANG_EDGE_CONDITION);
      uint32_t index = 0;
      for (const Expression* item : symbol.caseItemExpressions)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_CASE_ITEM, index++);
    }
    if constexpr (std::same_as<T, GenerateBlockArraySymbol>) {
      if (symbol.initialExpression)
        capture.semanticRole(id, symbol.initialExpression,
                             LLG_SLANG_EDGE_INITIALIZER);
      if (symbol.stopExpression)
        capture.semanticRole(id, symbol.stopExpression,
                             LLG_SLANG_EDGE_CONDITION);
      if (symbol.iterExpression)
        capture.semanticRole(id, symbol.iterExpression,
                             LLG_SLANG_EDGE_INCREMENT);
      if (symbol.loopVariable)
        capture.semanticRole(id, symbol.loopVariable,
                             LLG_SLANG_EDGE_DECLARATION);
    }
    if constexpr (std::same_as<T, ContinuousAssignSymbol>) {
      if (!symbol.getSyntax())
        capture.output.semantic_nodes[static_cast<size_t>(id)].subkind =
            LLG_SLANG_CONTINUOUS_ASSIGN_NET_DECL;
      capture.semanticRole(id, &symbol.getAssignment(), LLG_SLANG_EDGE_BODY);
      if (const TimingControl* delay = symbol.getDelay())
        capture.semanticRole(id, delay, LLG_SLANG_EDGE_DELAY);
    }
    if constexpr (std::same_as<T, PrimitiveInstanceSymbol>) {
      uint32_t index = 0;
      const auto ports = symbol.getPortConnections();
      for (const Expression* port : ports) {
        if (port) {
          capture.semanticRole(id, port, LLG_SLANG_EDGE_ACTUAL, index);
          auto& actual = capture.output.semantic_nodes[static_cast<size_t>(
              capture.ensureSemantic(port))];
          if (symbol.primitiveType.primitiveKind == PrimitiveSymbol::NInput)
            actual.flags |= index == 0 ? LLG_SLANG_SEMANTIC_OUTPUT
                                       : LLG_SLANG_SEMANTIC_INPUT;
          else if (symbol.primitiveType.primitiveKind == PrimitiveSymbol::NOutput)
            actual.flags |= index + 1 == ports.size()
                ? LLG_SLANG_SEMANTIC_INPUT
                : LLG_SLANG_SEMANTIC_OUTPUT;
          else if (index < symbol.primitiveType.ports.size())
            addDirection(actual, symbol.primitiveType.ports[index]->direction);
        }
        index++;
      }
      if (const TimingControl* delay = symbol.getDelay())
        capture.semanticRole(id, delay, LLG_SLANG_EDGE_DELAY);
    }
  }

  void addConnectionLabelBindings(const InstanceSymbol& symbol) {
    const syntax::SyntaxNode* syntaxNode = symbol.getSyntax();
    if (!syntaxNode || syntaxNode->kind != syntax::SyntaxKind::HierarchicalInstance)
      return;
    const auto& instanceSyntax = syntaxNode->as<syntax::HierarchicalInstanceSyntax>();
    const auto semanticConnections = symbol.getPortConnections();
    uint32_t orderedIndex = 0;
    for (const syntax::PortConnectionSyntax* syntaxConnection :
         instanceSyntax.connections) {
      if (syntaxConnection->kind != syntax::SyntaxKind::NamedPortConnection) {
        if (syntaxConnection->kind == syntax::SyntaxKind::WildcardPortConnection)
          continue;
        if (syntaxConnection->kind == syntax::SyntaxKind::OrderedPortConnection) {
          const auto& ordered =
              syntaxConnection->as<syntax::OrderedPortConnectionSyntax>();
          capture.connectionActual(ordered.expr->sourceRange());
        }
        else if (syntaxConnection->kind !=
                 syntax::SyntaxKind::EmptyPortConnection) {
          throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                              "unknown Slang port connection syntax");
        }
        if (orderedIndex < semanticConnections.size()) {
          const PortConnection* connection = semanticConnections[orderedIndex];
          auto& portNode = capture.output.semantic_nodes[static_cast<size_t>(
              capture.ensureSemantic(&connection->port))];
          portNode.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_PRESENT;
          if (!connection->getExpression())
            portNode.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_OPEN;
        }
        orderedIndex++;
        continue;
      }
      const auto& named = syntaxConnection->as<syntax::NamedPortConnectionSyntax>();
      if (named.expr)
        capture.connectionActual(named.expr->sourceRange());
      for (const PortConnection* connection : semanticConnections) {
        if (connection->port.name == named.name.valueText()) {
          const uint64_t portId = capture.declarationOnly
              ? captureReferenceTarget(connection->port)
              : capture.ensureSemantic(&connection->port);
          auto& portNode =
              capture.output.semantic_nodes[static_cast<size_t>(portId)];
          portNode.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_PRESENT;
          if (!connection->getExpression())
            portNode.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_OPEN;
          capture.lexicalBinding(named.name, portId,
                                 LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL);
          break;
        }
      }
    }

    const syntax::SyntaxNode* parent = instanceSyntax.parent;
    if (!parent || parent->kind != syntax::SyntaxKind::HierarchyInstantiation)
      return;
    const auto& hierarchy = parent->as<syntax::HierarchyInstantiationSyntax>();
    capture.lexicalBinding(hierarchy.type,
                           capture.ensureSemantic(&symbol.getDefinition()),
                           LLG_SLANG_LEXICAL_ROLE_REFERENCE);
    const auto* assignments = hierarchy.parameters;
    if (!assignments)
      return;
    for (const syntax::ParamAssignmentSyntax* assignment :
         assignments->parameters) {
      if (assignment->kind != syntax::SyntaxKind::NamedParamAssignment)
        continue;
      const auto& named = assignment->as<syntax::NamedParamAssignmentSyntax>();
      for (const ParameterSymbolBase* parameter : symbol.body.getParameters()) {
        if (parameter->symbol.name == named.name.valueText()) {
          capture.lexicalBinding(named.name,
                                 capture.declarationOnly
                                     ? captureReferenceTarget(parameter->symbol)
                                     : capture.ensureSemantic(&parameter->symbol),
                                 LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL);
          break;
        }
      }
    }
  }

  template<std::derived_from<Expression> T>
  void addExpressionRoles(const T& expression, uint64_t id) {
    if constexpr (std::same_as<T, UnaryExpression>) {
      capture.semanticRole(id, &expression.operand(), LLG_SLANG_EDGE_OPERAND);
    }
    else if constexpr (std::same_as<T, BinaryExpression>) {
      // Compound-assignment RHS expressions contain Slang's synthetic
      // LValueReferenceExpression placeholder. It is not an executable
      // operand and is intentionally omitted from the owned graph.
      if (!isSyntheticLValue(expression.left()))
        capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LEFT);
      capture.semanticRole(id, &expression.right(), LLG_SLANG_EDGE_RIGHT);
    }
    else if constexpr (std::same_as<T, AssignmentExpression>) {
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LHS);
      if (const Expression* rhs = compoundAssignmentSourceRhs(expression))
        capture.semanticRole(id, rhs, LLG_SLANG_EDGE_RHS);
      else {
        capture.output.semantic_nodes[static_cast<size_t>(id)].flags |=
            LLG_SLANG_SEMANTIC_BAD;
      }
      if (expression.timingControl)
        capture.semanticRole(id, expression.timingControl, LLG_SLANG_EDGE_DELAY);
    }
    else if constexpr (std::same_as<T, ConversionExpression>) {
      capture.semanticRole(id, &expression.operand(), LLG_SLANG_EDGE_OPERAND);
    }
    else if constexpr (std::same_as<T, ElementSelectExpression>) {
      capture.semanticRole(id, &expression.value(), LLG_SLANG_EDGE_BASE);
      capture.semanticRole(id, &expression.selector(), LLG_SLANG_EDGE_INDEX);
    }
    else if constexpr (std::same_as<T, RangeSelectExpression>) {
      capture.semanticRole(id, &expression.value(), LLG_SLANG_EDGE_BASE);
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LEFT);
      capture.semanticRole(id, &expression.right(), LLG_SLANG_EDGE_RIGHT);
    }
    else if constexpr (std::same_as<T, MemberAccessExpression>) {
      capture.semanticRole(id, &expression.value(), LLG_SLANG_EDGE_BASE);
      capture.semanticRole(id, &expression.member, LLG_SLANG_EDGE_REFERENCE);
    }
    else if constexpr (std::same_as<T, ConditionalExpression>) {
      for (uint32_t i = 0; i < expression.conditions.size(); i++)
        capture.semanticRole(id, expression.conditions[i].expr,
                             LLG_SLANG_EDGE_CONDITION, i);
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_THEN);
      capture.semanticRole(id, &expression.right(), LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, InsideExpression>) {
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LHS);
      uint32_t index = 0;
      for (const Expression* item : expression.rangeList())
        capture.semanticRole(id, item, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, ConcatenationExpression>) {
      uint32_t index = 0;
      for (const Expression* operand : expression.operands())
        capture.semanticRole(id, operand, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, ReplicationExpression>) {
      capture.semanticRole(id, &expression.count(), LLG_SLANG_EDGE_WIDTH);
      capture.semanticRole(id, &expression.concat(), LLG_SLANG_EDGE_OPERAND);
    }
    else if constexpr (std::same_as<T, StreamingConcatenationExpression>) {
      uint32_t index = 0;
      for (const auto& stream : expression.streams()) {
        capture.semanticRole(id, stream.operand, LLG_SLANG_EDGE_OPERAND, index);
        if (stream.withExpr)
          capture.semanticRole(id, stream.withExpr, LLG_SLANG_EDGE_INDEX, index);
        index++;
      }
    }
    else if constexpr (std::same_as<T, ReplicatedAssignmentPatternExpression>) {
      capture.semanticRole(id, &expression.count(), LLG_SLANG_EDGE_WIDTH);
      uint32_t index = 0;
      for (const Expression* element : expression.elements())
        capture.semanticRole(id, element, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, StructuredAssignmentPatternExpression>) {
      addStructuredPatternRoles(expression, id);
    }
    else if constexpr (std::same_as<T, NewArrayExpression>) {
      capture.semanticRole(id, &expression.sizeExpr(), LLG_SLANG_EDGE_WIDTH);
      if (const Expression* initializer = expression.initExpr())
        capture.semanticRole(id, initializer, LLG_SLANG_EDGE_INITIALIZER);
    }
    else if constexpr (std::same_as<T, NewClassExpression>) {
      if (expression.isSuperClass)
        capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary |=
            LLG_SLANG_NEW_CLASS_SUPER;
      if (const Expression* constructor = expression.constructorCall())
        capture.semanticRole(id, constructor, LLG_SLANG_EDGE_INITIALIZER);
    }
    else if constexpr (std::same_as<T, MinTypMaxExpression>) {
      capture.semanticRole(id, &expression.min(), LLG_SLANG_EDGE_OPERAND, 0);
      capture.semanticRole(id, &expression.typ(), LLG_SLANG_EDGE_OPERAND, 1);
      capture.semanticRole(id, &expression.max(), LLG_SLANG_EDGE_OPERAND, 2);
    }
    else if constexpr (std::same_as<T, ValueRangeExpression>) {
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LEFT);
      capture.semanticRole(id, &expression.right(), LLG_SLANG_EDGE_RIGHT);
    }
    else if constexpr (std::same_as<T, CallExpression>) {
      const bool systemMethod = isSystemMethodCall(expression);
      if (expression.thisClass())
        capture.semanticRole(id, expression.thisClass(), LLG_SLANG_EDGE_RECEIVER);
      uint32_t index = 0;
      auto arguments = expression.arguments();
      if (systemMethod) {
        capture.semanticRole(id, arguments.front(), LLG_SLANG_EDGE_RECEIVER);
        arguments = arguments.subspan(1);
      }
      for (const Expression* argument : arguments)
        capture.semanticRole(id, argument, LLG_SLANG_EDGE_ARGUMENT, index++);
      if (!expression.isSystemCall()) {
        const SubroutineSymbol* callee = std::get<0>(expression.subroutine);
        if (callee) {
          const uint64_t target = capture.ensureSemantic(callee);
          capture.output.semantic_nodes[static_cast<size_t>(id)].target_id = target;
          capture.semanticEdge(id, LLG_SLANG_EDGE_CALLEE, target);
        }
      }
      if (isSuperMethodCall(expression))
        capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary |=
            LLG_SLANG_CALL_SUPER;
    }
    else if constexpr (std::same_as<T, AssertionInstanceExpression>) {
      capture.semanticRole(id, &expression.body, LLG_SLANG_EDGE_BODY);
      uint32_t index = 0;
      for (const auto& [formal, actual] : expression.arguments) {
        capture.semanticEdge(id, LLG_SLANG_EDGE_ASSERTION_FORMAL,
                             capture.ensureSemantic(formal), index);
        std::visit([&](auto* value) {
          if (value)
            capture.semanticRole(id, value, LLG_SLANG_EDGE_ASSERTION_ACTUAL,
                                 index);
        }, actual);
        index++;
      }
    }
  }

  void initializePatternKey(uint64_t keyId, uint64_t patternId,
                            uint32_t subkind, std::string_view name,
                            const Expression& value) {
    auto& key = capture.output.semantic_nodes[static_cast<size_t>(keyId)];
    key.parent_id = patternId;
    key.kind = LLG_SLANG_SEMANTIC_EXPRESSION;
    key.subkind = subkind;
    key.operation = LLG_SLANG_OP_NONE;
    key.name = storeString(capture.output, name);
    key.detail = storeString(capture.output, "AssignmentPatternKey");
    key.range = capture.span(value.sourceRange);
    key.type_id = capture.type(*value.type);
  }

  void addStructuredPatternRoles(
      const StructuredAssignmentPatternExpression& expression, uint64_t id) {
    capture.removeChildEdges(id);
    uint32_t index = 0;
    for (const auto& setter : expression.memberSetters) {
      const uint64_t keyId = capture.ensureSemantic(&setter);
      initializePatternKey(keyId, id, LLG_SLANG_EXPR_PATTERN_MEMBER_KEY,
                           setter.member->name, *setter.expr);
      capture.semanticRole(keyId, setter.expr, LLG_SLANG_EDGE_BODY);
      capture.semanticRole(keyId, setter.member, LLG_SLANG_EDGE_DECLARATION);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, keyId, index++);
    }
    for (const auto& setter : expression.typeSetters) {
      const uint64_t keyId = capture.ensureSemantic(&setter);
      initializePatternKey(keyId, id, LLG_SLANG_EXPR_PATTERN_TYPE_KEY,
                           setter.type->toString(), *setter.expr);
      capture.output.semantic_nodes[static_cast<size_t>(keyId)].type_id =
          capture.type(*setter.type);
      capture.semanticRole(keyId, setter.expr, LLG_SLANG_EDGE_BODY);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, keyId, index++);
    }
    for (const auto& setter : expression.indexSetters) {
      const uint64_t keyId = capture.ensureSemantic(&setter);
      initializePatternKey(
          keyId, id, LLG_SLANG_EXPR_PATTERN_INDEX_KEY,
          capture.sourceText(setter.index->sourceRange), *setter.expr);
      capture.semanticRole(keyId, setter.index, LLG_SLANG_EDGE_INDEX);
      capture.semanticRole(keyId, setter.expr, LLG_SLANG_EDGE_BODY);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, keyId, index++);
    }
    if (expression.defaultSetter) {
      const uint64_t keyId = capture.ensureSemantic(&expression.defaultSetter);
      initializePatternKey(keyId, id, LLG_SLANG_EXPR_PATTERN_DEFAULT_KEY,
                           "default", *expression.defaultSetter);
      capture.semanticRole(keyId, expression.defaultSetter, LLG_SLANG_EDGE_BODY);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, keyId, index);
    }
  }

  template<std::derived_from<Statement> T>
  void addStatementRoles(const T& statement, uint64_t id) {
    if constexpr (std::same_as<T, ExpressionStatement>) {
      capture.semanticRole(id, &statement.expr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, BlockStatement>) {
      capture.semanticRole(id, &statement.body, LLG_SLANG_EDGE_BODY);
      if (statement.blockSymbol)
        capture.semanticRole(id, statement.blockSymbol,
                             LLG_SLANG_EDGE_DECLARATION);
    }
    else if constexpr (std::same_as<T, ConditionalStatement>) {
      for (uint32_t i = 0; i < statement.conditions.size(); i++)
        capture.semanticRole(id, statement.conditions[i].expr,
                             LLG_SLANG_EDGE_CONDITION, i);
      capture.semanticRole(id, &statement.ifTrue, LLG_SLANG_EDGE_THEN);
      if (statement.ifFalse)
        capture.semanticRole(id, statement.ifFalse, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, ImmediateAssertionStatement>) {
      capture.semanticRole(id, &statement.cond, LLG_SLANG_EDGE_CONDITION);
      if (statement.ifTrue)
        capture.semanticRole(id, statement.ifTrue, LLG_SLANG_EDGE_THEN);
      if (statement.ifFalse)
        capture.semanticRole(id, statement.ifFalse, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, ConcurrentAssertionStatement>) {
      capture.semanticRole(id, &statement.propertySpec,
                           LLG_SLANG_EDGE_PROPERTY_SPEC);
      if (statement.ifTrue)
        capture.semanticRole(id, statement.ifTrue, LLG_SLANG_EDGE_THEN);
      if (statement.ifFalse)
        capture.semanticRole(id, statement.ifFalse, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, CaseStatement>) {
      capture.semanticRole(id, &statement.expr, LLG_SLANG_EDGE_CASE_EXPRESSION);
      uint32_t itemIndex = 0;
      for (const auto& item : statement.items) {
        uint32_t expressionIndex = 0;
        for (const Expression* expression : item.expressions) {
          const uint32_t combined = (itemIndex << 16) |
                                    std::min<uint32_t>(expressionIndex, UINT16_MAX);
          capture.semanticRole(id, expression, LLG_SLANG_EDGE_CASE_ITEM, combined);
          expressionIndex++;
        }
        capture.semanticRole(id, item.stmt, LLG_SLANG_EDGE_BRANCH, itemIndex++);
      }
      if (statement.defaultCase)
        capture.semanticRole(id, statement.defaultCase, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, ForLoopStatement>) {
      uint32_t index = 0;
      for (const Expression* initializer : statement.initializers)
        capture.semanticRole(id, initializer, LLG_SLANG_EDGE_INITIALIZER, index++);
      if (statement.stopExpr)
        capture.semanticRole(id, statement.stopExpr, LLG_SLANG_EDGE_CONDITION);
      index = 0;
      for (const Expression* step : statement.steps)
        capture.semanticRole(id, step, LLG_SLANG_EDGE_INCREMENT, index++);
      capture.semanticRole(id, &statement.body, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, ForeachLoopStatement>) {
      // Keep the source list length even when the last dimensions are
      // omitted. Declaration edges carry their own dimension index; this
      // scalar records the number of slots so the owned DB can preserve
      // trailing omissions without exposing Slang AST pointers.
      capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary =
          statement.loopDims.size();
      capture.semanticRole(id, &statement.arrayRef, LLG_SLANG_EDGE_BASE);
      uint32_t index = 0;
      for (const auto& dimension : statement.loopDims) {
        if (dimension.loopVar)
          capture.semanticRole(id, dimension.loopVar,
                               LLG_SLANG_EDGE_DECLARATION, index);
        index++;
      }
      capture.semanticRole(id, &statement.body, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, VariableDeclStatement>) {
      capture.semanticRole(id, &statement.symbol, LLG_SLANG_EDGE_DECLARATION);
      if (const Expression* initializer = statement.symbol.getInitializer())
        capture.semanticRole(id, initializer, LLG_SLANG_EDGE_INITIALIZER);
    }
    else if constexpr (std::same_as<T, RepeatLoopStatement>) {
      capture.semanticRole(id, &statement.count, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &statement.body, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, WhileLoopStatement> ||
                       std::same_as<T, DoWhileLoopStatement>) {
      capture.semanticRole(id, &statement.cond, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &statement.body, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, ForeverLoopStatement>) {
      capture.semanticRole(id, &statement.body, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, TimedStatement>) {
      capture.semanticRole(id, &statement.timing, LLG_SLANG_EDGE_EVENT);
      capture.semanticRole(id, &statement.stmt, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, WaitStatement>) {
      capture.semanticRole(id, &statement.cond, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &statement.stmt, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, WaitOrderStatement>) {
      uint32_t index = 0;
      for (const Expression* event : statement.events)
        capture.semanticRole(id, event, LLG_SLANG_EDGE_EVENT, index++);
      if (statement.ifTrue)
        capture.semanticRole(id, statement.ifTrue, LLG_SLANG_EDGE_THEN);
      if (statement.ifFalse)
        capture.semanticRole(id, statement.ifFalse, LLG_SLANG_EDGE_ELSE);
      // Wait and wait_order intentionally share the stable ABI subkind. The
      // auxiliary marker preserves the distinction without spending a new
      // semantic flag bit or leaking Slang enum values across the boundary.
      capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary = 1;
    }
    else if constexpr (std::same_as<T, ReturnStatement>) {
      if (statement.expr)
        capture.semanticRole(id, statement.expr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, EventTriggerStatement>) {
      capture.semanticRole(id, &statement.target, LLG_SLANG_EDGE_EVENT);
      capture.output.semantic_nodes[static_cast<size_t>(id)].target_id =
          capture.ensureSemantic(&statement.target);
      if (statement.timing)
        capture.semanticRole(id, statement.timing, LLG_SLANG_EDGE_DELAY);
      if (statement.isNonBlocking)
        capture.output.semantic_nodes[static_cast<size_t>(id)].flags |=
            LLG_SLANG_SEMANTIC_NONBLOCKING;
    }
    else if constexpr (std::same_as<T, ProceduralAssignStatement>) {
      if (statement.assignment.kind == ExpressionKind::Assignment) {
        const auto& assignment =
            statement.assignment.template as<AssignmentExpression>();
        capture.semanticRole(id, &assignment.left(), LLG_SLANG_EDGE_LHS);
        capture.semanticRole(id, &assignment.right(), LLG_SLANG_EDGE_RHS);
      }
      else {
        capture.semanticRole(id, &statement.assignment, LLG_SLANG_EDGE_BODY);
        capture.output.semantic_nodes[static_cast<size_t>(id)].flags |=
            LLG_SLANG_SEMANTIC_BAD;
      }
    }
    else if constexpr (std::same_as<T, ProceduralDeassignStatement>) {
      capture.semanticRole(id, &statement.lvalue, LLG_SLANG_EDGE_LHS);
    }
    else if constexpr (std::same_as<T, DisableStatement>) {
      capture.semanticRole(id, &statement.target, LLG_SLANG_EDGE_REFERENCE);
      if (const Symbol* target = statement.target.getSymbolReference())
        capture.output.semantic_nodes[static_cast<size_t>(id)].target_id =
            capture.ensureSemantic(target);
    }
  }

  template<typename T>
  void addAuxiliaryRoles(const T& node, uint64_t id) {
    if constexpr (std::same_as<T, DelayControl>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_DELAY);
    }
    else if constexpr (std::same_as<T, Delay3Control>) {
      capture.semanticRole(id, &node.expr1, LLG_SLANG_EDGE_DELAY, 0);
      if (node.expr2)
        capture.semanticRole(id, node.expr2, LLG_SLANG_EDGE_DELAY, 1);
      if (node.expr3)
        capture.semanticRole(id, node.expr3, LLG_SLANG_EDGE_DELAY, 2);
    }
    else if constexpr (std::same_as<T, SignalEventControl>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_EVENT);
      if (node.iffCondition)
        capture.semanticRole(id, node.iffCondition, LLG_SLANG_EDGE_CONDITION);
    }
    else if constexpr (std::same_as<T, EventListControl>) {
      uint32_t index = 0;
      for (const TimingControl* event : node.events)
        capture.semanticRole(id, event, LLG_SLANG_EDGE_EVENT, index++);
    }
    else if constexpr (std::same_as<T, RepeatedEventControl>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &node.event, LLG_SLANG_EDGE_EVENT);
    }
    else if constexpr (std::same_as<T, CycleDelayControl>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_DELAY);
    }
    else if constexpr (std::same_as<T, SimpleAssertionExpr>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_OPERAND);
    }
    else if constexpr (std::same_as<T, InvalidAssertionExpr>) {
      if (node.child)
        capture.semanticRole(id, node.child, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, SequenceConcatExpr>) {
      uint32_t index = 0;
      for (const auto& element : node.elements)
        capture.semanticSequenceRole(id, element.sequence, LLG_SLANG_EDGE_OPERAND,
                                     index++, element.delay);
    }
    else if constexpr (std::same_as<T, SequenceWithMatchExpr>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_BODY);
      uint32_t index = 0;
      for (const Expression* item : node.matchItems)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, UnaryAssertionExpr>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, BinaryAssertionExpr>) {
      capture.semanticRole(id, &node.left, LLG_SLANG_EDGE_LEFT);
      capture.semanticRole(id, &node.right, LLG_SLANG_EDGE_RIGHT);
    }
    else if constexpr (std::same_as<T, FirstMatchAssertionExpr>) {
      capture.semanticRole(id, &node.seq, LLG_SLANG_EDGE_BODY);
      uint32_t index = 0;
      for (const Expression* item : node.matchItems)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, ClockingAssertionExpr>) {
      capture.semanticRole(id, &node.clocking, LLG_SLANG_EDGE_CLOCKING);
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, StrongWeakAssertionExpr>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, AbortAssertionExpr>) {
      capture.semanticRole(id, &node.condition, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, ConditionalAssertionExpr>) {
      capture.semanticRole(id, &node.condition, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &node.ifExpr, LLG_SLANG_EDGE_THEN);
      if (node.elseExpr)
        capture.semanticRole(id, node.elseExpr, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, CaseAssertionExpr>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_CASE_EXPRESSION);
      uint32_t itemIndex = 0;
      for (const auto& item : node.items) {
        uint32_t expressionIndex = 0;
        for (const Expression* expression : item.expressions) {
          const uint32_t combined = (itemIndex << 16) |
                                    std::min<uint32_t>(expressionIndex, UINT16_MAX);
          capture.semanticRole(id, expression, LLG_SLANG_EDGE_CASE_ITEM, combined);
          expressionIndex++;
        }
        capture.semanticRole(id, item.body, LLG_SLANG_EDGE_BRANCH, itemIndex++);
      }
      if (node.defaultCase)
        capture.semanticRole(id, node.defaultCase, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, DisableIffAssertionExpr>) {
      capture.semanticRole(id, &node.condition, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_BODY);
    }
  }
};

uint32_t lexicalKindForSemantic(const LlgSlangSemanticNode& node) {
  const std::string_view detail(
      reinterpret_cast<const char*>(node.detail.data), node.detail.len);
  if (detail == "Genvar")
    return LLG_SLANG_LEXICAL_GENVAR;
  switch (node.kind) {
    case LLG_SLANG_SEMANTIC_DEFINITION:
      if (node.flags & LLG_SLANG_SEMANTIC_INTERFACE)
        return LLG_SLANG_LEXICAL_INTERFACE;
      if (node.flags & LLG_SLANG_SEMANTIC_PROGRAM)
        return LLG_SLANG_LEXICAL_PROGRAM;
      return LLG_SLANG_LEXICAL_MODULE;
    case LLG_SLANG_SEMANTIC_PACKAGE: return LLG_SLANG_LEXICAL_PACKAGE;
    case LLG_SLANG_SEMANTIC_CLASS: return LLG_SLANG_LEXICAL_CLASS;
    case LLG_SLANG_SEMANTIC_PORT: return LLG_SLANG_LEXICAL_PORT;
    case LLG_SLANG_SEMANTIC_NET: return LLG_SLANG_LEXICAL_NET;
    case LLG_SLANG_SEMANTIC_VARIABLE: return LLG_SLANG_LEXICAL_VARIABLE;
    case LLG_SLANG_SEMANTIC_PARAMETER: return LLG_SLANG_LEXICAL_PARAMETER;
    case LLG_SLANG_SEMANTIC_ARGUMENT: return LLG_SLANG_LEXICAL_VARIABLE;
    case LLG_SLANG_SEMANTIC_SUBROUTINE: return LLG_SLANG_LEXICAL_FUNCTION;
    case LLG_SLANG_SEMANTIC_ENUM_CONSTANT: return LLG_SLANG_LEXICAL_ENUM_MEMBER;
    default: return LLG_SLANG_LEXICAL_IDENTIFIER;
  }
}

class LexicalCapture final : public syntax::SyntaxVisitor<LexicalCapture> {
public:
  explicit LexicalCapture(Capture& capture) : capture(capture) {}

  void handle(const syntax::NamedPortConnectionSyntax& syntax) {
    capture.lexicalBinding(syntax.name, LLG_SLANG_INVALID_ID,
                           LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL,
                           LLG_SLANG_LEXICAL_PORT);
    visitDefault(syntax);
  }

  void handle(const syntax::NamedParamAssignmentSyntax& syntax) {
    capture.lexicalBinding(syntax.name, LLG_SLANG_INVALID_ID,
                           LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL,
                           LLG_SLANG_LEXICAL_PARAMETER);
    if (syntax.expr)
      capture.connectionActual(syntax.expr->sourceRange());
    visitDefault(syntax);
  }

  void visitToken(parsing::Token token) {
    for (const parsing::Trivia& trivia : token.trivia()) {
      for (parsing::Token skipped : trivia.getSkippedTokens())
        addToken(skipped, LLG_SLANG_LEXICAL_SKIPPED);
      if (syntax::SyntaxNode* directive = trivia.syntax();
          directive && directives.insert(directive).second) {
        directiveDepth++;
        directive->visit(*this);
        directiveDepth--;
      }
    }
    addToken(token, 0);
  }

  void bindSemanticTokens() {
    std::sort(capture.output.lexical_tokens.begin(),
              capture.output.lexical_tokens.end(),
              [](const auto& left, const auto& right) {
                if (left.range.file_id != right.range.file_id)
                  return left.range.file_id < right.range.file_id;
                if (left.range.start != right.range.start)
                  return left.range.start < right.range.start;
                return left.range.end < right.range.end;
              });
    using Position = std::pair<uint64_t, uint64_t>;
    std::map<Position, uint64_t> declarations;
    std::map<Position, uint64_t> referencesByStart;
    std::map<Position, uint64_t> referencesByEnd;
    std::map<std::tuple<uint64_t, uint64_t, uint64_t>,
             Capture::LexicalBinding> connectionLabels;
    std::sort(capture.connectionActualRanges.begin(),
              capture.connectionActualRanges.end(), [](const auto& left,
                                                        const auto& right) {
                return std::tie(left.file_id, left.start, left.end) <
                       std::tie(right.file_id, right.start, right.end);
              });
    for (const auto& binding : capture.lexicalBindings) {
      if (binding.range.file_id == LLG_SLANG_INVALID_ID)
        continue;
      const auto key = std::tuple{binding.range.file_id, binding.range.start,
                                  binding.range.end};
      auto [it, inserted] = connectionLabels.try_emplace(key, binding);
      if (!inserted &&
          it->second.semantic_id == LLG_SLANG_INVALID_ID &&
          binding.semantic_id != LLG_SLANG_INVALID_ID) {
        it->second = binding;
      }
    }
    for (const auto& node : capture.output.semantic_nodes) {
      if (node.range.file_id == LLG_SLANG_INVALID_ID)
        continue;
      const uint64_t lexicalTarget =
          capture.canonicalLexicalTarget(node.id);
      bool sourceGenvarClone =
          lexicalTarget != node.id &&
          lexicalTarget < capture.output.semantic_nodes.size() &&
          capture.output.semantic_nodes[static_cast<size_t>(lexicalTarget)]
                  .subkind == LLG_SLANG_VARIABLE_GENVAR;
      for (uint64_t edgeIndex = 0; edgeIndex < node.edge_count; edgeIndex++) {
        const auto& edge = capture.output.semantic_edges[
            static_cast<size_t>(node.edge_start + edgeIndex)];
        if (edge.role == LLG_SLANG_EDGE_SOURCE_IDENTITY &&
            edge.target_id < capture.output.semantic_nodes.size() &&
            capture.output.semantic_nodes[static_cast<size_t>(edge.target_id)]
                    .subkind == LLG_SLANG_VARIABLE_GENVAR) {
          sourceGenvarClone = true;
          break;
        }
      }
      if (!sourceGenvarClone && node.kind != LLG_SLANG_SEMANTIC_EXPRESSION &&
          node.name.len != 0) {
        const Position position{node.range.file_id, node.range.start};
        auto [it, inserted] = declarations.try_emplace(position, node.id);
        if (!inserted && node.kind == LLG_SLANG_SEMANTIC_DEFINITION &&
            capture.output.semantic_nodes[static_cast<size_t>(it->second)].kind !=
                LLG_SLANG_SEMANTIC_DEFINITION) {
          it->second = node.id;
        }
      }
      if ((node.subkind == LLG_SLANG_EXPR_REFERENCE ||
           node.subkind == LLG_SLANG_EXPR_MEMBER_ACCESS) &&
          node.target_id != LLG_SLANG_INVALID_ID) {
        const uint64_t lexicalTarget =
            capture.canonicalLexicalTarget(node.target_id);
        const std::string_view detail(
            reinterpret_cast<const char*>(node.detail.data), node.detail.len);
        if (node.subkind == LLG_SLANG_EXPR_REFERENCE &&
            detail != "HierarchicalValue") {
          referencesByStart.try_emplace(
              Position{node.range.file_id, node.range.start}, lexicalTarget);
        }
        referencesByEnd.try_emplace(Position{node.range.file_id, node.range.end},
                                    lexicalTarget);
      }
    }

    for (auto& token : capture.output.lexical_tokens) {
      if (token.kind != LLG_SLANG_LEXICAL_IDENTIFIER ||
          token.range.file_id == LLG_SLANG_INVALID_ID)
        continue;
      uint64_t semanticId = LLG_SLANG_INVALID_ID;
      uint32_t role = LLG_SLANG_LEXICAL_ROLE_NONE;
      uint32_t boundKind = LLG_SLANG_LEXICAL_UNKNOWN;
      if (auto it = connectionLabels.find(
              {token.range.file_id, token.range.start, token.range.end});
          it != connectionLabels.end()) {
        // Scoped syntax lookups can name a port's internal net / variable.
        // Use the same source identity as expression-derived references below.
        semanticId = capture.canonicalLexicalTarget(it->second.semantic_id);
        role = it->second.role;
        boundKind = it->second.kind;
      }
      else if (auto it = declarations.find(
                   {token.range.file_id, token.range.start});
               it != declarations.end()) {
        const auto& declaration = capture.output.semantic_nodes[
            static_cast<size_t>(it->second)];
        const std::string_view tokenText(
            reinterpret_cast<const char*>(token.text.data), token.text.len);
        const std::string_view declarationName(
            reinterpret_cast<const char*>(declaration.name.data),
            declaration.name.len);
        if (tokenText == declarationName) {
          semanticId = it->second;
          role = LLG_SLANG_LEXICAL_ROLE_DECLARATION;
        }
      }
      if (role == LLG_SLANG_LEXICAL_ROLE_NONE) {
        if (auto it = referencesByStart.find(
                   {token.range.file_id, token.range.start});
               it != referencesByStart.end()) {
          semanticId = it->second;
          role = LLG_SLANG_LEXICAL_ROLE_REFERENCE;
        }
        else if (auto it = referencesByEnd.find(
                   {token.range.file_id, token.range.end});
               it != referencesByEnd.end()) {
          semanticId = it->second;
          role = LLG_SLANG_LEXICAL_ROLE_REFERENCE;
        }
        else {
          const std::string_view tokenText(
              reinterpret_cast<const char*>(token.text.data), token.text.len);
          for (const auto& scope : capture.genvarLexicalScopes) {
            if (scope.range.file_id == token.range.file_id &&
                scope.range.start <= token.range.start &&
                scope.range.end >= token.range.end && scope.name == tokenText) {
              semanticId = scope.semantic_id;
              role = LLG_SLANG_LEXICAL_ROLE_REFERENCE;
              break;
            }
          }
        }
      }
      const std::string_view tokenText(
          reinterpret_cast<const char*>(token.text.data), token.text.len);
      for (const auto& scope : capture.genvarLexicalScopes) {
        if (scope.range.file_id != token.range.file_id ||
            scope.range.start > token.range.start ||
            scope.range.end < token.range.end || scope.name != tokenText) {
          continue;
        }
        const bool generatedLoopTarget =
            semanticId < capture.output.semantic_nodes.size() &&
            (capture.output.semantic_nodes[static_cast<size_t>(semanticId)]
                 .flags & LLG_SLANG_SEMANTIC_IMPLICIT) != 0;
        if (role == LLG_SLANG_LEXICAL_ROLE_NONE || generatedLoopTarget) {
          semanticId = scope.semantic_id;
          role = LLG_SLANG_LEXICAL_ROLE_REFERENCE;
          boundKind = LLG_SLANG_LEXICAL_GENVAR;
        }
        break;
      }
      if (role == LLG_SLANG_LEXICAL_ROLE_REFERENCE) {
        const auto actual = std::upper_bound(
            capture.connectionActualRanges.begin(),
            capture.connectionActualRanges.end(), token.range,
            [](const auto& key, const auto& range) {
              return std::tie(key.file_id, key.start) <
                     std::tie(range.file_id, range.start);
            });
        if (actual != capture.connectionActualRanges.begin()) {
          const auto& range = *std::prev(actual);
          if (range.file_id == token.range.file_id &&
              range.start <= token.range.start && range.end >= token.range.end) {
            role = LLG_SLANG_LEXICAL_ROLE_CONNECTION_ACTUAL;
          }
        }
      }
      if (role != LLG_SLANG_LEXICAL_ROLE_NONE) {
        token.role = role;
        if (boundKind != LLG_SLANG_LEXICAL_UNKNOWN)
          token.kind = boundKind;
      }
      if (semanticId != LLG_SLANG_INVALID_ID &&
          semanticId < capture.output.semantic_nodes.size()) {
        const auto& semantic = capture.output.semantic_nodes[semanticId];
        token.semantic_id = semanticId;
        token.kind = lexicalKindForSemantic(semantic);
      }
    }
  }

private:
  Capture& capture;
  std::unordered_set<const syntax::SyntaxNode*> directives;
  uint32_t directiveDepth = 0;

  void addToken(parsing::Token token, uint32_t extraFlags) {
    const std::string_view text = token.rawText();
    if (text.empty() && !token.isMissing())
      return;
    if (capture.output.lexical_tokens.size() >= capture.maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "lexical token limit exceeded");

    uint32_t kind = LLG_SLANG_LEXICAL_OPERATOR;
    if (token.kind == parsing::TokenKind::Identifier ||
        token.kind == parsing::TokenKind::SystemIdentifier)
      kind = LLG_SLANG_LEXICAL_IDENTIFIER;
    else if (parsing::LexerFacts::isKeyword(token.kind))
      kind = LLG_SLANG_LEXICAL_KEYWORD;
    else if (token.kind == parsing::TokenKind::StringLiteral)
      kind = LLG_SLANG_LEXICAL_STRING;
    else if (token.kind == parsing::TokenKind::IntegerLiteral ||
             token.kind == parsing::TokenKind::IntegerBase ||
             token.kind == parsing::TokenKind::RealLiteral ||
             token.kind == parsing::TokenKind::TimeLiteral ||
             token.kind == parsing::TokenKind::UnbasedUnsizedLiteral)
      kind = LLG_SLANG_LEXICAL_NUMBER;

    uint32_t flags = extraFlags;
    // Directive replacement text is not executable source until expanded.
    if (directiveDepth != 0)
      flags |= LLG_SLANG_LEXICAL_DIRECTIVE;
    if (token.isMissing())
      flags |= LLG_SLANG_LEXICAL_MISSING;
    if (token.location().valid() &&
        capture.sourceManager.isMacroLoc(token.location()))
      flags |= LLG_SLANG_LEXICAL_MACRO_EXPANSION;
    if (kind == LLG_SLANG_LEXICAL_IDENTIFIER &&
        (directiveDepth != 0 ||
         (flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION) != 0))
      kind = LLG_SLANG_LEXICAL_MACRO;
    chargeRecord(capture.output, sizeof(LlgSlangLexicalToken));
    capture.output.lexical_tokens.push_back({
        capture.span(token.range()), kind,
        kind == LLG_SLANG_LEXICAL_KEYWORD ? LLG_SLANG_LEXICAL_ROLE_KEYWORD
                                         : LLG_SLANG_LEXICAL_ROLE_NONE,
        flags, 0, LLG_SLANG_INVALID_ID, storeString(capture.output, text)});
  }
};

class CaptureClient final : public DiagnosticClient {
public:
  CaptureClient(Capture& capture, uint32_t provider)
      : capture(capture), provider(provider) {}

  void issue(DiagnosticEngine& diagnosticEngine, const Diagnostic& diagnostic) {
    root = &diagnostic;
    rootIndex = LLG_SLANG_INVALID_ID;
    if (diagnostic.isError() && diagnostic.code != diag::AlwaysWithoutTimingControl)
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

bool matchesQualifiedParameter(std::string_view requested,
                               std::string_view owner,
                               std::string_view parameter) {
  return requested.size() == owner.size() + parameter.size() + 1 &&
         requested.starts_with(owner) && requested[owner.size()] == '.' &&
         requested.substr(owner.size() + 1) == parameter;
}

bool isKnownTopParameter(const RootSymbol& root, std::string_view requested) {
  for (const InstanceSymbol* top : root.topInstances) {
    for (const ParameterSymbolBase* parameter : top->body.getParameters()) {
      const std::string_view name = parameter->symbol.name;
      if (requested == name || matchesQualifiedParameter(requested, top->name, name) ||
          matchesQualifiedParameter(requested, top->getDefinition().name, name))
        return true;
    }
  }
  return false;
}

void captureNavigation(Compilation& compilation, Capture& capture) {
  SemanticCapture visitor(capture);
  compilation.getRoot().visit(visitor);
  std::unordered_map<const Symbol*, const InstanceSymbol*> sourceInstances;
  for (const Symbol* definition : compilation.getDefinitions()) {
    definition->visit(visitor);
    if (definition->kind != SymbolKind::Definition)
      continue;
    auto& instance = InstanceSymbol::createInvalid(
        compilation, definition->as<DefinitionSymbol>());
    // Source instances live in the compilation arena but are not root members.
    instance.setParent(*definition->getParentScope());
    instance.visit(visitor);
    sourceInstances.emplace(definition, &instance);
  }
  visitor.bindSourceConnections(compilation, sourceInstances);
}

std::unique_ptr<LlgSlangSnapshot> compileImpl(const LlgSlangCompileRequest& request) {
  if (request.abi_version != LLG_SLANG_ABI_VERSION)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unsupported Slang ABI version");
  constexpr uint32_t knownFlags =
      LLG_SLANG_COMPILE_LIBRARY_UNITS |
      LLG_SLANG_COMPILE_EDITION_VERILOG_2001 |
      LLG_SLANG_COMPILE_EDITION_SYSTEMVERILOG_2009 |
      LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS;
  if ((request.flags & ~knownFlags) != 0)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unknown compile request flags");
  const EditionPolicy edition = editionPolicy(request.flags);
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
  if (request.system_subroutine_count != 0 && request.system_subroutines == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "system_subroutines has a null pointer");

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
  if (request.system_subroutine_count > kHardMaxSystemSubroutines)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "system subroutine count limit exceeded");

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
  compilationOptions.languageVersion = edition.languageVersion;
  if ((request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0)
    compilationOptions.flags |= CompilationFlags::IgnoreUninstantiatedModules;
  compilationOptions.defaultTimeScale = TimeScale(
      TimeScaleValue(TimeUnit::Nanoseconds, TimeScaleMagnitude::One),
      TimeScaleValue(TimeUnit::Picoseconds, TimeScaleMagnitude::One));
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

  std::vector<std::shared_ptr<driver::UserDefinedSubroutine>> userDefinedSubroutines;
  userDefinedSubroutines.reserve(static_cast<size_t>(request.system_subroutine_count));
  for (uint64_t i = 0; i < request.system_subroutine_count; i++) {
    const std::string_view prototype = checkedView(
        request.system_subroutines[i], "system subroutine prototype");
    if (prototype.empty() || prototype.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "system subroutine prototypes must be nonempty and contain no NUL bytes");
    addChecked(configBytes, prototype.size(), kHardMaxConfigBytes,
               "configuration byte");
    auto result = driver::UserDefinedSubroutine::create(prototype, sourceManager);
    if (!result)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT, result.error());
    userDefinedSubroutines.emplace_back(std::move(*result));
  }

  std::vector<SourceBuffer> buffers;
  buffers.reserve(static_cast<size_t>(request.source_count));
  for (uint64_t i = 0; i < request.source_count; i++)
    buffers.push_back(sourceManager.assignText(sourcePaths[i], sourceTexts[i]));

  parsing::PreprocessorOptions preprocessorOptions;
  preprocessorOptions.languageVersion = edition.languageVersion;
  preprocessorOptions.predefines = std::move(predefines);
  preprocessorOptions.additionalIncludePaths = std::move(includeDirs);
  preprocessorOptions.keywordMapping.reserve(sourcePaths.size());
  for (const std::string& path : sourcePaths)
    preprocessorOptions.keywordMapping.emplace_back(path, edition.keywordVersion);
  parsing::LexerOptions lexerOptions;
  lexerOptions.languageVersion = edition.languageVersion;
  parsing::ParserOptions parserOptions;
  parserOptions.languageVersion = edition.languageVersion;
  Bag parseOptions;
  parseOptions.set(std::move(preprocessorOptions));
  parseOptions.set(std::move(lexerOptions));
  parseOptions.set(std::move(parserOptions));
  Bag compileOptions;
  compileOptions.set(std::move(compilationOptions));
  Compilation compilation(compileOptions);
  for (const auto& subroutine : userDefinedSubroutines)
    compilation.addSystemSubroutine(subroutine);
  bool anyCompilationUnit = false;
  if ((request.flags & LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS) != 0) {
    std::vector<SourceBuffer> compilationBuffers;
    compilationBuffers.reserve(static_cast<size_t>(request.source_count));
    for (uint64_t i = 0; i < request.source_count; i++) {
      if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) != 0)
        compilationBuffers.push_back(buffers[static_cast<size_t>(i)]);
    }
    anyCompilationUnit = !compilationBuffers.empty();
    if (anyCompilationUnit) {
      auto tree = syntax::SyntaxTree::fromBuffers(
          compilationBuffers, sourceManager, parseOptions);
      if ((request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0)
        tree->isLibraryUnit = true;
      compilation.addSyntaxTree(std::move(tree));
    }
  } else {
    for (uint64_t i = 0; i < request.source_count; i++) {
      if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) == 0)
        continue;
      anyCompilationUnit = true;
      auto tree = syntax::SyntaxTree::fromBuffer(
          buffers[static_cast<size_t>(i)], sourceManager, parseOptions);
      if ((request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0)
        tree->isLibraryUnit = true;
      compilation.addSyntaxTree(std::move(tree));
    }
  }
  if (!anyCompilationUnit)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "at least one compilation unit source is required");

  auto output = std::make_unique<LlgSlangSnapshot>();
  output->flags |= edition.snapshotFlag;
  if ((request.flags & LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS) != 0)
    output->flags |= LLG_SLANG_SNAPSHOT_MERGED_COMPILATION_UNITS;
  output->output_byte_limit = effectiveLimit(request.limits.max_output_bytes,
      kDefaultMaxOutputBytes, kHardMaxOutputBytes);
  Capture capture{*output, sourceManager, request.limits,
                  (request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0};
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

  // Capture source bodies before diagnostics are cached so their errors are included.
  if (capture.declarationOnly)
    captureNavigation(compilation, capture);
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
  for (uint64_t i = 0; i < request.parameter_override_count; i++) {
    const std::string_view name =
        checkedView(request.parameter_overrides[i].name,
                    "parameter override name");
    if (!isKnownTopParameter(root, name)) {
      capture.addIntegrationDiagnostic(
          "UnknownParameterOverride",
          "unknown top-level parameter override '" + std::string(name) + "'");
    }
  }
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
    engine.setSeverity(diag::AlwaysWithoutTimingControl, DiagnosticSeverity::Warning);
    for (const Diagnostic& diagnostic : diagnostics)
      analysisClient->issue(engine, diagnostic);
    output->flags |= LLG_SLANG_SNAPSHOT_ANALYSIS_RAN;
  }

  if (!capture.declarationOnly) {
    SemanticCapture semanticCapture(capture);
    root.visit(semanticCapture);
    for (const Symbol* definition : compilation.getDefinitions())
      definition->visit(semanticCapture);
  }
  capture.markOverriddenParameters();
  capture.finalizeSourceIdentities();
  capture.finalizeSemanticEdges();

  LexicalCapture lexicalCapture(capture);
  for (const auto& tree : compilation.getSyntaxTrees())
    tree->root().visit(lexicalCapture);
  lexicalCapture.bindSemanticTokens();

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
      dataOrNull(snapshot->semantic_nodes),
      static_cast<uint64_t>(snapshot->semantic_nodes.size()),
      dataOrNull(snapshot->semantic_edges),
      static_cast<uint64_t>(snapshot->semantic_edges.size()),
      dataOrNull(snapshot->lexical_tokens),
      static_cast<uint64_t>(snapshot->lexical_tokens.size()),
      dataOrNull(snapshot->type_ranges),
      static_cast<uint64_t>(snapshot->type_ranges.size()),
      dataOrNull(snapshot->type_members),
      static_cast<uint64_t>(snapshot->type_members.size()),
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
