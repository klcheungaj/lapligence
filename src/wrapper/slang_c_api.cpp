#include "slang_c_api.h"

#include <algorithm>
#include <bit>
#include <cctype>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <deque>
#include <span>
#include <exception>
#include <filesystem>
#include <iterator>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <tuple>
#include <utility>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include "slang_platform.hpp"

#include "slang/analysis/AnalysisManager.h"
#include "slang/ast/ASTVisitor.h"
#include "slang/ast/Bitstream.h"
#include "slang/ast/EvalContext.h"
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
#include "slang/diagnostics/CompilationDiags.h"
#include "slang/diagnostics/DeclarationsDiags.h"
#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/diagnostics/DiagnosticClient.h"
#include "slang/diagnostics/ExpressionsDiags.h"
#include "slang/diagnostics/StatementsDiags.h"
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

class ProfileStage {
public:
  explicit ProfileStage(const char* name) : name(name) {
    const char* value = std::getenv("LLG_PROFILE_STAGES");
    enabled = value && std::strcmp(value, "1") == 0;
    if (enabled) {
      std::fprintf(stderr, "llg-profile begin %s\n", name);
      started = std::chrono::steady_clock::now();
    }
  }
  void finish() {
    if (enabled) {
      const double seconds = std::chrono::duration<double>(
          std::chrono::steady_clock::now() - started).count();
      std::fprintf(stderr, "llg-profile end %s seconds=%.6f\n", name, seconds);
      enabled = false;
    }
  }
  ~ProfileStage() { finish(); }
  ProfileStage(const ProfileStage&) = delete;
  ProfileStage& operator=(const ProfileStage&) = delete;

private:
  const char* name;
  bool enabled;
  std::chrono::steady_clock::time_point started;
};

// Zero-request defaults keep small budgets for callers that do not choose
// their own. Hard ceilings clamp every request and are structural only, so a
// caller (the batch simulator) may admit a whole design bounded by memory:
// - record counts stop at UINT32_MAX because the Rust snapshot stores node,
//   type, constant and edge-window references as 32-bit compact IDs;
// - source buffers stop at 2^24 because Slang's 28-bit buffer IDs are shared
//   with macro-expansion locations and wrap silently;
// - byte and value-bit budgets have no ceiling beyond 64-bit arithmetic.
// Keep these in sync with the NATIVE_* constants in src/ffi/slang.rs.
constexpr uint64_t kMaxIndexedRecords = UINT32_MAX;
constexpr uint64_t kUnboundedBytes = UINT64_MAX;
constexpr uint64_t kDefaultMaxSources = 256;
constexpr uint64_t kHardMaxSources = uint64_t{1} << 24;
constexpr uint64_t kDefaultMaxSourceBytes = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxSourceBytes = kUnboundedBytes;
constexpr uint64_t kDefaultMaxDiagnostics = 10000;
constexpr uint64_t kHardMaxDiagnostics = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxInstances = 100000;
constexpr uint64_t kHardMaxInstances = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxParameters = 500000;
constexpr uint64_t kHardMaxParameters = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxConstants = 1000000;
constexpr uint64_t kHardMaxConstants = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxTypes = 100000;
constexpr uint64_t kHardMaxTypes = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxValueBits = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxValueBits = kUnboundedBytes;
constexpr uint64_t kDefaultMaxRelatedDiagnostics = 80000;
constexpr uint64_t kHardMaxRelatedDiagnostics = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxOutputBytes = 64 * 1024 * 1024;
constexpr uint64_t kHardMaxOutputBytes = kUnboundedBytes;

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
constexpr uint64_t kHardMaxSemanticNodes = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxSemanticEdges = 4000000;
constexpr uint64_t kHardMaxSemanticEdges = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxLexicalTokens = 4000000;
constexpr uint64_t kHardMaxLexicalTokens = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxTypeRanges = 1000000;
constexpr uint64_t kHardMaxTypeRanges = kMaxIndexedRecords;
constexpr uint64_t kDefaultMaxTypeMembers = 1000000;
constexpr uint64_t kHardMaxTypeMembers = kMaxIndexedRecords;
constexpr uint64_t kHardMaxDefines = 4096;
constexpr uint64_t kHardMaxTopModules = 4096;
constexpr uint64_t kHardMaxIncludeDirs = 4096;
constexpr uint64_t kHardMaxParameterOverrides = 4096;
constexpr uint64_t kHardMaxSystemSubroutines = 4096;
constexpr uint64_t kHardMaxConfigBytes = 4 * 1024 * 1024;
// Logical name Slang gives the buffer of a parsed parameter override value.
constexpr std::string_view kCommandLineBufferName = "<command-line>";

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
                        std::string(description) + " limit exceeded (limit=" +
                            std::to_string(limit) + ")");
  total += amount;
}

// A capture budget failure that names the effective limit, so callers can
// identify the exhausted budget and its effective ceiling.
[[noreturn]] void throwCountLimit(const char* description, uint64_t limit) {
  throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                      std::string(description) + " limit exceeded (limit=" +
                          std::to_string(limit) + ")");
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

// Records per semantic-node chunk; also the stream batch for that table.
constexpr size_t kSemanticNodeChunkRecords = 4096;

// Append-only table stored in fixed-size chunks. Growth never moves
// records, so capture has no reallocation copy spike and references stay
// valid across push_back; the stream releases each chunk once delivered.
template<typename T, size_t ChunkRecords>
class ChunkedVector {
public:
  size_t size() const { return count; }
  T& operator[](size_t index) {
    return chunks[index / ChunkRecords][index % ChunkRecords];
  }
  const T& operator[](size_t index) const {
    return chunks[index / ChunkRecords][index % ChunkRecords];
  }
  void push_back(const T& value) {
    if (count % ChunkRecords == 0)
      chunks.emplace_back(new T[ChunkRecords]);
    chunks.back()[count % ChunkRecords] = value;
    count++;
  }
  size_t chunkCount() const { return chunks.size(); }
  const T* chunkData(size_t chunk) const { return chunks[chunk].get(); }
  size_t chunkSize(size_t chunk) const {
    return chunk + 1 < chunks.size() ? ChunkRecords : count - chunk * ChunkRecords;
  }
  // Frees one chunk; the table must not be indexed afterwards.
  void releaseChunk(size_t chunk) { chunks[chunk].reset(); }

private:
  std::vector<std::unique_ptr<T[]>> chunks;
  size_t count = 0;
};

struct SnapshotStringHash {
  using is_transparent = void;
  size_t operator()(std::string_view value) const noexcept {
    return std::hash<std::string_view>{}(value);
  }
};

struct CaptureOutput {
  uint32_t flags = 0;
  uint64_t output_bytes = 0;
  uint64_t output_byte_limit = kDefaultMaxOutputBytes;
  std::unordered_set<std::string, SnapshotStringHash, std::equal_to<>> strings;
  std::vector<LlgSlangFile> files;
  std::vector<LlgSlangDiagnostic> diagnostics;
  std::vector<LlgSlangRelatedDiagnostic> related;
  std::vector<LlgSlangInstance> instances;
  std::vector<LlgSlangParameter> parameters;
  std::vector<LlgSlangType> types;
  std::vector<LlgSlangConstant> constants;
  std::vector<uint64_t> value_words;
  ChunkedVector<LlgSlangSemanticNode, kSemanticNodeChunkRecords> semantic_nodes;
  std::vector<LlgSlangSemanticEdge> semantic_edges;
  std::vector<LlgSlangLexicalToken> lexical_tokens;
  std::vector<LlgSlangTypeRange> type_ranges;
  std::vector<LlgSlangTypeMember> type_members;
  std::vector<LlgSlangUdpTable> udp_tables;
  std::vector<LlgSlangUdpRow> udp_rows;
  std::vector<LlgSlangSourceLibrary> source_libraries;
  std::vector<LlgSlangLineDirective> line_directives;
  std::vector<LlgSlangEditionFinding> edition_findings;
  std::vector<LlgSlangProtectedEnvelope> protected_envelopes;
  std::vector<LlgSlangFsmPragma> fsm_pragmas;
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

LlgSlangString storeString(CaptureOutput& snapshot, std::string_view value) {
  if (value.empty())
    return {nullptr, 0};
  addChecked(snapshot.output_bytes, value.size(), snapshot.output_byte_limit,
             "export byte");
  auto found = snapshot.strings.find(value);
  if (found == snapshot.strings.end())
    found = snapshot.strings.emplace(value).first;
  const auto& stored = *found;
  return {reinterpret_cast<const uint8_t*>(stored.data()),
          static_cast<uint64_t>(stored.size())};
}

void chargeRecord(CaptureOutput& snapshot, uint64_t bytes) {
  if (bytes > snapshot.output_byte_limit ||
      snapshot.output_bytes > snapshot.output_byte_limit - bytes) {
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
        "export byte limit exceeded (limit=" +
        std::to_string(snapshot.output_byte_limit) +
        ", bytes=" + std::to_string(snapshot.output_bytes) +
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
    // The range is a name in a `define body, bound from one expansion of it;
    // every expansion of the macro binds the same range.
    bool macroBody = false;
  };

  struct GenvarLexicalScope {
    LlgSlangSourceRange range;
    std::string name;
    uint64_t semantic_id;
  };

  CaptureOutput& output;
  const SourceManager& sourceManager;
  const LlgSlangLimits& limits;
  std::vector<std::pair<BufferID, uint64_t>> fileIds;
  std::unordered_map<const Type*, uint64_t> typeIds;
  // Independently declared equivalent virtual-interface types need one owned
  // identity. Canonical pointers alone are only unique for nominal classes.
  std::unordered_map<const DefinitionSymbol*, std::vector<const VirtualInterfaceType*>>
      virtualInterfaceTypes;
  std::unordered_map<const void*, uint64_t> semanticIds;
  // DPI-C export declarations by the syntax node that holds them, each as
  // (exported subroutine name, C identifier), filled on first use. Every
  // instance of a declaring scope shares that scope's syntax, so this finds
  // the export of each instance's subroutine; the compilation's own export
  // list keeps one subroutine per C identifier. Views borrow syntax-tree
  // tokens, which outlive capture.
  std::unordered_map<const syntax::SyntaxNode*,
                     std::vector<std::pair<std::string_view, std::string_view>>>
      dpiExportDirectives;
  std::vector<std::vector<LlgSlangSemanticEdge>> pendingEdges;
  // Small nodes retain cheap scans. Only high-fanout parents pay for indexes;
  // positions refer to the authoritative ordered edge vector, never its memory.
  static constexpr size_t kIndexedSemanticEdgeThreshold = 64;
  // Dimensions walked to reach the typedef of a declared type; a type that
  // nests more is reported without a typedef name.
  static constexpr uint64_t kMaxTypedefDimensions = 64;
  static uint64_t semanticEdgeKey(uint32_t role, uint32_t index) {
    return (static_cast<uint64_t>(role) << 32) | index;
  }
  struct SemanticEdgeIndex {
    std::map<uint64_t, size_t> roles;
    std::unordered_map<uint64_t, std::vector<size_t>> children;

    void add(const LlgSlangSemanticEdge& edge, size_t position) {
      roles.emplace(semanticEdgeKey(edge.role, edge.index), position);
      if (edge.role == LLG_SLANG_EDGE_CHILD)
        children[edge.target_id].push_back(position);
    }
  };
  std::unordered_map<uint64_t, SemanticEdgeIndex> indexedSemanticEdges;
  std::vector<LexicalBinding> lexicalBindings;
  std::vector<LlgSlangSourceRange> connectionActualRanges;
  std::vector<GenvarLexicalScope> genvarLexicalScopes;
  // Struct and union member declaration names from the syntax. A member that
  // nothing references has no semantic node, so its name would stay unbound.
  std::vector<LlgSlangSourceRange> fieldDeclarations;
  std::unordered_map<uint64_t, uint64_t> lexicalTargetAliases;
  std::unordered_map<const syntax::SyntaxNode*, std::vector<uint64_t>>
      sourceIdentityGroups;
  std::unordered_set<const ParameterSymbol*> overriddenParameters;
  std::unordered_map<uint64_t, SourceLocation> unitValueDeclarations;
  // Parameter override spellings the bridge rewrote for Slang, so
  // diagnostics show what the user wrote: rewritten -> given, for whole
  // `NAME=VALUE` options and for VALUE texts.
  std::unordered_map<std::string, std::string> givenOverrideOptions;
  std::unordered_map<std::string, std::string> givenOverrideValues;
  uint64_t valueBits = 0;
  uint64_t semanticEdgeCount = 0;
  bool declarationOnly = false;

  Capture(CaptureOutput& output, const SourceManager& sourceManager,
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

  void captureUdpTable(uint64_t primitiveId, const PrimitiveSymbol& primitive) {
    // A primitive with fewer than two ports has no table to evaluate; Slang
    // reports it (PrimitiveTwoPorts), so capture must not fail first.
    if (primitive.primitiveKind != PrimitiveSymbol::UserDefined ||
        primitive.ports.size() < 2)
      return;
    if (output.udp_tables.size() >= maxSemanticNodes())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "UDP table limit exceeded");
    const uint64_t capturedRows = output.udp_rows.size();
    const uint64_t rowCount = primitive.table.size();
    if (capturedRows > maxSemanticEdges() ||
        rowCount > maxSemanticEdges() - capturedRows)
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "UDP row limit exceeded");
    const uint64_t inputCount = primitive.ports.size() - 1;
    if (inputCount > std::numeric_limits<uint32_t>::max())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "UDP input count exceeds the ABI limit");
    // Slang appends one table entry per syntax entry unless the entry has an
    // error, so equal counts pair each kept row with its source entry.
    const syntax::UdpBodySyntax* body = nullptr;
    if (const auto* syntax = primitive.getSyntax();
        syntax && syntax->kind == syntax::SyntaxKind::UdpDeclaration)
      body = syntax->as<syntax::UdpDeclarationSyntax>().body;
    const bool rowsHaveSyntax = body && body->entries.size() == rowCount;
    const uint64_t rowStart = output.udp_rows.size();
    for (size_t index = 0; index < primitive.table.size(); ++index) {
      const auto& row = primitive.table[index];
      chargeRecord(output, sizeof(LlgSlangUdpRow));
      output.udp_rows.push_back({
          storeString(output, row.inputs),
          static_cast<uint32_t>(static_cast<unsigned char>(row.state)),
          static_cast<uint32_t>(static_cast<unsigned char>(row.output)),
          row.isEdgeSensitive ? LLG_SLANG_UDP_ROW_EDGE_SENSITIVE : 0u,
          0u,
          rowsHaveSyntax ? span(body->entries[index]->sourceRange())
                         : LlgSlangSourceRange{LLG_SLANG_INVALID_ID, 0, 0},
      });
    }
    uint32_t initialValue = 0;
    if (primitive.isSequential && primitive.initVal &&
        primitive.initVal->isInteger()) {
      // Slang accepts only 0, 1 and 1'bx here (PrimitiveInitVal otherwise).
      const auto& value = primitive.initVal->integer();
      if (value.getBitWidth() == 1 && exactlyEqual(value[0], logic_t::x))
        initialValue = 'x';
      else if (value == 0)
        initialValue = '0';
      else if (value == 1)
        initialValue = '1';
    }
    chargeRecord(output, sizeof(LlgSlangUdpTable));
    output.udp_tables.push_back({
        primitiveId,
        static_cast<uint32_t>(inputCount),
        initialValue,
        rowStart,
        rowCount,
        storeString(output, primitive.name),
    });
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

  // The written range of a macro-expanded token whose text comes from the
  // actual arguments of the outermost macro usage: its fully original
  // location lies in the use-site buffer at or after the use site, and the
  // source there spells the token (a pasted or stringified token is new text
  // and keeps the use site). Body text is written in a `define, which always
  // precedes its use in the same buffer or lies in another buffer.
  // `std::nullopt` for every other location.
  std::optional<LlgSlangSourceRange> macroArgumentSpan(SourceLocation location,
                                                       std::string_view text) const {
    if (!location.valid() || !sourceManager.isMacroLoc(location) || text.empty())
      return std::nullopt;
    const SourceLocation original = sourceManager.getFullyOriginalLoc(location);
    const SourceLocation expanded = sourceManager.getFullyExpandedLoc(location);
    if (!original.valid() || !expanded.valid() || !sourceManager.isFileLoc(original) ||
        original.buffer() != expanded.buffer() || original.offset() < expanded.offset())
      return std::nullopt;
    const std::string_view source = sourceManager.getSourceText(original.buffer());
    if (original.offset() > source.size() ||
        source.substr(original.offset(), text.size()) != text)
      return std::nullopt;
    const LlgSlangSourceRange range = span(original, text.size());
    if (range.file_id == LLG_SLANG_INVALID_ID)
      return std::nullopt;
    return range;
  }

  // The range in a `define body that a macro-expanded token was copied
  // from: its fully original location is a file location that spells the
  // token and is not text written in the actual arguments of the outermost
  // usage (`macroArgumentSpan`). Nested usages resolve through every level,
  // so a name written in an outer body and passed to an inner macro maps to
  // the outer body. A pasted or stringified token is new text and has none.
  std::optional<LlgSlangSourceRange> macroBodySpan(SourceLocation location,
                                                   std::string_view text) const {
    if (!location.valid() || !sourceManager.isMacroLoc(location) || text.empty() ||
        macroArgumentSpan(location, text))
      return std::nullopt;
    const SourceLocation original = sourceManager.getFullyOriginalLoc(location);
    if (!original.valid() || !sourceManager.isFileLoc(original))
      return std::nullopt;
    const std::string_view source = sourceManager.getSourceText(original.buffer());
    if (original.offset() > source.size() ||
        source.substr(original.offset(), text.size()) != text)
      return std::nullopt;
    const LlgSlangSourceRange range = span(original, text.size());
    if (range.file_id == LLG_SLANG_INVALID_ID)
      return std::nullopt;
    return range;
  }

  // Physical range of one token: its written argument text for a macro
  // argument token, its `define body text for a token copied from a macro
  // body, otherwise its span.
  LlgSlangSourceRange tokenSpan(parsing::Token token) const {
    if (auto written = macroArgumentSpan(token.location(), token.rawText()))
      return *written;
    if (auto body = macroBodySpan(token.location(), token.rawText()))
      return *body;
    return span(token.range());
  }

  // Byte offset + 1 of `location` when it lies in file `file` and below
  // 2^32 - 1, else 0: a keyword position a statement keeps in `auxiliary`.
  uint64_t keywordOffset(SourceLocation location, uint64_t file) const {
    if (!location.valid() || file == LLG_SLANG_INVALID_ID)
      return 0;
    const LlgSlangSourceRange at = span(location);
    if (at.file_id != file || at.start >= UINT32_MAX - 1)
      return 0;
    return at.start + 1;
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
    // A parameter override value is parsed from its own unadmitted buffer;
    // name the value the message is about.
    std::string message(diag.formattedMessage);
    if (primary.file_id == LLG_SLANG_INVALID_ID && diag.location.valid() &&
        sourceManager.getFileName(diag.location) == kCommandLineBufferName) {
      std::string_view text = sourceManager.getSourceText(diag.location.buffer());
      while (!text.empty() && text.back() == '\0')
        text.remove_suffix(1);
      std::string value(text);
      if (auto given = givenOverrideValues.find(value); given != givenOverrideValues.end())
        value = given->second;
      message = "parameter override value `" + value + "`: " + message;
    }
    else if (original.code == diag::InvalidParamOverrideOpt) {
      for (const auto& [rewritten, given] : givenOverrideOptions) {
        const std::string quoted = "'" + rewritten + "'";
        if (auto at = message.find(quoted); at != std::string::npos) {
          message.replace(at, quoted.size(), "'" + given + "'");
          break;
        }
      }
    }
    chargeRecord(output, sizeof(LlgSlangDiagnostic));
    output.diagnostics.push_back({
        provider,
        diagnosticSeverity(diag.severity),
        diagnosticSubsystem(original.code.getSubsystem()),
        original.code.getCode(),
        storeString(output, toString(original.code)),
        storeString(output, engine.getOptionName(original.code)),
        storeString(output, message),
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


  // A specialization of the built-in `std::mailbox #(T)` exports `T` as its
  // element type. The type parameter is resolved in the specializing scope,
  // so a typedef declared anywhere keeps its canonical shape; consumers must
  // not re-resolve the rendered parameter spelling. An untyped mailbox and
  // every other class, including a user class named `mailbox`, have none.
  uint64_t mailboxElementType(const ClassType& classType) {
    const GenericClassDefSymbol* generic = classType.genericClass;
    if (!generic || generic->name != "mailbox")
      return LLG_SLANG_INVALID_ID;
    const Scope* parent = generic->getParentScope();
    if (!parent || &parent->asSymbol() != &classType.getCompilation().getStdPackage())
      return LLG_SLANG_INVALID_ID;
    for (const Symbol* parameter : classType.genericParameters) {
      if (parameter && parameter->kind == SymbolKind::TypeParameter &&
          parameter->name == "T") {
        const Type& element = parameter->as<TypeParameterSymbol>().targetType.getType();
        return element.isUntypedType() ? LLG_SLANG_INVALID_ID : type(element);
      }
    }
    return LLG_SLANG_INVALID_ID;
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
      case SymbolKind::CovergroupType: kind = LLG_SLANG_TYPE_COVERGROUP; break;
      case SymbolKind::CHandleType: kind = LLG_SLANG_TYPE_CHANDLE; break;
      case SymbolKind::EventType: kind = LLG_SLANG_TYPE_EVENT; break;
      case SymbolKind::VoidType: kind = LLG_SLANG_TYPE_VOID; break;
      case SymbolKind::VirtualInterfaceType:
        kind = LLG_SLANG_TYPE_VIRTUAL_INTERFACE;
        break;
      case SymbolKind::DPIOpenArrayType:
        kind = canonical.as<DPIOpenArrayType>().isPacked
                   ? LLG_SLANG_TYPE_DPI_OPEN_PACKED
                   : LLG_SLANG_TYPE_DPI_OPEN_UNPACKED;
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
    if (canonical.isTaggedUnion())
      flags |= LLG_SLANG_TYPE_TAGGED;

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
    const uint64_t availableTypeMembers =
        maxTypeMembers() -
        std::min<uint64_t>(output.type_members.size(), maxTypeMembers());
    auto appendField = [&](const FieldSymbol* field) {
      if (fields.size() >= availableTypeMembers)
        throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                            "type member limit exceeded");
      fields.push_back(field);
    };
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
      case SymbolKind::DPIOpenArrayType:
        elementType = type(canonical.as<DPIOpenArrayType>().elementType);
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
      case SymbolKind::ClassType:
        elementType = mailboxElementType(canonical.as<ClassType>());
        break;
      case SymbolKind::PackedStructType:
        for (const FieldSymbol& field :
             canonical.as<PackedStructType>().membersOfType<FieldSymbol>())
          appendField(&field);
        break;
      case SymbolKind::PackedUnionType:
        for (const FieldSymbol& field :
             canonical.as<PackedUnionType>().membersOfType<FieldSymbol>())
          appendField(&field);
        break;
      case SymbolKind::UnpackedStructType:
        for (const FieldSymbol* field : canonical.as<UnpackedStructType>().fields)
          appendField(field);
        break;
      case SymbolKind::UnpackedUnionType:
        for (const FieldSymbol* field : canonical.as<UnpackedUnionType>().fields)
          appendField(field);
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
      uint64_t initializerId = LLG_SLANG_INVALID_ID;
      if (const Expression* initializer = field->getInitializer()) {
        ASTContext context(*field->getParentScope(), LookupLocation::before(*field));
        ConstantValue value = context.eval(*initializer);
        if (value.isUnpacked() && field->getType().isFixedSize()) {
          if (value.getBitstreamWidth() > maxValueBits() - std::min(valueBits, maxValueBits()))
            throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                                "member initializer constant bit limit exceeded");
          EvalContext evaluation(context);
          value = Bitstream::convertToBitVector(std::move(value), initializer->sourceRange,
                                               evaluation);
        }
        initializerId = constant(value);
      }
      const auto [typedefId, typedefDepth] = declaredTypedef(field->getType());
      members.push_back({storeString(output, field->name), type(field->getType()),
                         field->bitOffset, field->getType().getBitWidth(), initializerId,
                         typedefId, typedefDepth});
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

  // The typedef a declared type names (IEEE 1800-2009 37.23), as the
  // semantic node of its declaration and the number of packed and fixed
  // unpacked dimensions declared above it. Canonical type records drop
  // typedef names, so typespec names come only from these references.
  std::pair<uint64_t, uint64_t> declaredTypedef(const Type& declared) {
    const Type* current = &declared;
    for (uint64_t depth = 0; depth < kMaxTypedefDimensions; depth++) {
      if (current->isAlias())
        return {ensureSemantic(&current->as<TypeAliasType>()), depth};
      if (current->kind == SymbolKind::PackedArrayType)
        current = &current->as<PackedArrayType>().elementType;
      else if (current->kind == SymbolKind::FixedSizeUnpackedArrayType)
        current = &current->as<FixedSizeUnpackedArrayType>().elementType;
      else
        break;
    }
    return {LLG_SLANG_INVALID_ID, 0};
  }

  void typedefEdge(uint64_t source, const Type& declared) {
    const auto [target, depth] = declaredTypedef(declared);
    if (target != LLG_SLANG_INVALID_ID)
      semanticEdge(source, LLG_SLANG_EDGE_TYPEDEF, target, static_cast<uint32_t>(depth));
  }

  uint64_t constant(const ConstantValue& value) {
    if (output.constants.size() >= maxConstants())
      throwCountLimit("constant", maxConstants());
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
        if (syntax &&
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

  // Records the library that `%l` and configuration bindings report for one
  // scope node. Callers record each node once; the table is bounded by the
  // semantic node limit and charged like every other export record.
  void sourceLibrary(uint64_t id, const SourceLibrary* library,
                     std::string_view config = {}) {
    if (!library)
      return;
    chargeRecord(output, sizeof(LlgSlangSourceLibrary));
    output.source_libraries.push_back({id, storeString(output, library->name),
                                       storeString(output, config)});
  }

  uint64_t ensureSemantic(const void* identity) {
    if (auto it = semanticIds.find(identity); it != semanticIds.end())
      return it->second;
    if (output.semantic_nodes.size() >= maxSemanticNodes())
      throwCountLimit("semantic node", maxSemanticNodes());
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
      throwCountLimit("semantic node", maxSemanticNodes());
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

  SemanticEdgeIndex* semanticEdgeIndex(uint64_t source) {
    const auto& edges = pendingEdges[static_cast<size_t>(source)];
    if (edges.size() < kIndexedSemanticEdgeThreshold)
      return nullptr;
    auto [it, inserted] = indexedSemanticEdges.try_emplace(source);
    if (inserted) {
      for (size_t position = 0; position < edges.size(); position++)
        it->second.add(edges[position], position);
    }
    return &it->second;
  }

  LlgSlangSemanticEdge* findSemanticEdge(uint64_t source, uint32_t role,
                                        uint32_t index) {
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    if (auto* lookup = semanticEdgeIndex(source)) {
      const auto it = lookup->roles.find(semanticEdgeKey(role, index));
      return it == lookup->roles.end() ? nullptr : &edges[it->second];
    }
    for (auto& edge : edges) {
      if (edge.role == role && edge.index == index)
        return &edge;
    }
    return nullptr;
  }

  LlgSlangSemanticEdge* findSemanticChild(uint64_t source, uint64_t target) {
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    if (auto* lookup = semanticEdgeIndex(source)) {
      const auto it = lookup->children.find(target);
      return it == lookup->children.end() ? nullptr : &edges[it->second.front()];
    }
    for (auto& edge : edges) {
      if (edge.role == LLG_SLANG_EDGE_CHILD && edge.target_id == target)
        return &edge;
    }
    return nullptr;
  }

  uint32_t nextSemanticChildIndex(uint64_t source) {
    if (auto* lookup = semanticEdgeIndex(source)) {
      // Exclude UINT32_MAX, whose +1 wraps to zero in the original scan.
      auto it = lookup->roles.lower_bound(
          semanticEdgeKey(LLG_SLANG_EDGE_CHILD, UINT32_MAX));
      if (it != lookup->roles.begin()) {
        --it;
        if ((it->first >> 32) == LLG_SLANG_EDGE_CHILD)
          return static_cast<uint32_t>(it->first) + 1;
      }
      return 0;
    }
    uint32_t index = 0;
    for (const auto& edge : pendingEdges[static_cast<size_t>(source)]) {
      if (edge.role == LLG_SLANG_EDGE_CHILD)
        index = std::max(index, edge.index + 1);
    }
    return index;
  }

  void semanticEdge(uint64_t source, uint32_t role, uint64_t target,
                    uint32_t index = 0) {
    if (source == LLG_SLANG_INVALID_ID || target == LLG_SLANG_INVALID_ID)
      return;
    if (const auto* edge = findSemanticEdge(source, role, index)) {
      if (edge->target_id == target)
        return;
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "conflicting semantic edge role and index");
    }
    if (semanticEdgeCount >= maxSemanticEdges())
      throwCountLimit("semantic edge", maxSemanticEdges());
    chargeRecord(output, sizeof(LlgSlangSemanticEdge));
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    edges.push_back({role, index, target});
    if (auto it = indexedSemanticEdges.find(source);
        it != indexedSemanticEdges.end())
      it->second.add(edges.back(), edges.size() - 1);
    semanticEdgeCount++;
  }

  void semanticChild(uint64_t parent, uint64_t child, bool deduplicate = false) {
    if (deduplicate && findSemanticChild(parent, child))
      return;
    semanticEdge(parent, LLG_SLANG_EDGE_CHILD, child,
                 nextSemanticChildIndex(parent));
  }

  void semanticRole(uint64_t source, const void* targetIdentity, uint32_t role,
                    uint32_t index = 0) {
    const uint64_t target = ensureSemantic(targetIdentity);
    auto& edges = pendingEdges[static_cast<size_t>(source)];
    if (const auto* existing = findSemanticEdge(source, role, index)) {
      if (existing->target_id != target)
        throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                            "conflicting semantic edge role and index");
      if (!findSemanticChild(source, target))
        return;
      const size_t oldSize = edges.size();
      edges.erase(std::remove_if(edges.begin(), edges.end(),
          [=](const LlgSlangSemanticEdge& edge) {
            return edge.role == LLG_SLANG_EDGE_CHILD &&
                   edge.target_id == target;
          }), edges.end());
      if (oldSize != edges.size())
        indexedSemanticEdges.erase(source);
      semanticEdgeCount -= oldSize - edges.size();
      return;
    }
    if (auto* edge = findSemanticChild(source, target)) {
      if (auto it = indexedSemanticEdges.find(source);
          it != indexedSemanticEdges.end()) {
        auto& lookup = it->second;
        lookup.roles.erase(semanticEdgeKey(edge->role, edge->index));
        auto& positions = lookup.children.at(target);
        const size_t position = positions.front();
        if (role != LLG_SLANG_EDGE_CHILD) {
          positions.erase(positions.begin());
          if (positions.empty())
            lookup.children.erase(target);
        }
        lookup.roles.emplace(semanticEdgeKey(role, index), position);
      }
      edge->role = role;
      edge->index = index;
      return;
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
    auto* edge = findSemanticEdge(source, role, index);
    if (!edge)
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "sequence edge metadata has no edge");
    edge->sequence_delay_min = range.min;
    edge->sequence_delay_max = range.max.value_or(LLG_SLANG_ASSERTION_RANGE_UNBOUNDED);
    edge->sequence_delay_valid = 1;
  }

  void replaceChildRoles(uint64_t source, uint32_t role) {
    indexedSemanticEdges.erase(source);
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
    if (oldSize != edges.size())
      indexedSemanticEdges.erase(source);
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
      std::vector<LlgSlangSemanticEdge>().swap(pendingEdges[i]);
    }
    std::vector<std::vector<LlgSlangSemanticEdge>>().swap(pendingEdges);
    indexedSemanticEdges.clear();
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

  // Keyed by the token's physical range, so a name written in a macro
  // argument binds at its written text and a name copied from a macro body
  // binds at the body text (see `tokenSpan`).
  void lexicalBinding(parsing::Token token, uint64_t semanticId,
                      uint32_t role, uint32_t kind = LLG_SLANG_LEXICAL_UNKNOWN) {
    if (auto body = macroBodySpan(token.location(), token.rawText())) {
      lexicalBinding(*body, semanticId, role, kind, true);
      // The expansion copy keeps its use-site binding as well. Body text is
      // directive text, which the edition checks skip; a compilation-unit
      // forward reference is decided at the use site, where it expands.
      lexicalBinding(span(token.range()), semanticId, role, kind);
      return;
    }
    lexicalBinding(tokenSpan(token), semanticId, role, kind);
  }

  // `range` must be exactly one name token's range.
  void lexicalBinding(SourceRange range, uint64_t semanticId, uint32_t role,
                      uint32_t kind = LLG_SLANG_LEXICAL_UNKNOWN) {
    lexicalBinding(span(range), semanticId, role, kind);
  }

  void lexicalBinding(const LlgSlangSourceRange& range, uint64_t semanticId,
                      uint32_t role, uint32_t kind = LLG_SLANG_LEXICAL_UNKNOWN,
                      bool macroBody = false) {
    if (lexicalBindings.size() >= maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "lexical binding limit exceeded");
    lexicalBindings.push_back({range, semanticId, role, kind, macroBody});
  }

  void connectionActual(SourceRange range) {
    if (connectionActualRanges.size() >= maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "connection actual range limit exceeded");
    const auto physical = span(range);
    if (physical.file_id != LLG_SLANG_INVALID_ID)
      connectionActualRanges.push_back(physical);
  }

  void fieldDeclaration(parsing::Token token) {
    if (fieldDeclarations.size() >= maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "field declaration limit exceeded");
    const auto physical = tokenSpan(token);
    if (physical.file_id != LLG_SLANG_INVALID_ID)
      fieldDeclarations.push_back(physical);
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

// The C identifier of the DPI-C export declaration naming `symbol`, if any
// (IEEE 1800-2009 35.4: the subroutine name when no c_identifier is given).
// An export declaration sits in the scope that defines the subroutine
// (35.7), so it is a sibling of the subroutine's syntax; the frontend has
// already rejected exports in any other scope and kind mismatches.
std::optional<std::string_view> dpiExportName(Capture& capture,
                                              const SubroutineSymbol& symbol) {
  const syntax::SyntaxNode* own = symbol.getSyntax();
  const syntax::SyntaxNode* holder = own ? own->parent.get() : nullptr;
  if (!holder)
    return std::nullopt;
  auto [entry, inserted] = capture.dpiExportDirectives.try_emplace(holder);
  if (inserted) {
    for (size_t index = 0, count = holder->getChildCount(); index < count; ++index) {
      const syntax::SyntaxNode* child = holder->childNode(index);
      if (!child || child->kind != syntax::SyntaxKind::DPIExport)
        continue;
      const auto& directive = child->as<syntax::DPIExportSyntax>();
      const std::string_view name = directive.name.valueText();
      const std::string_view cName = directive.c_identifier.isMissing()
          ? name
          : directive.c_identifier.valueText();
      entry->second.emplace_back(name, cName.empty() ? name : cName);
    }
  }
  for (const auto& [name, cName] : entry->second) {
    if (name == symbol.name)
      return cName;
  }
  return std::nullopt;
}

uint32_t semanticSymbolKind(SymbolKind kind) {
  switch (kind) {
    case SymbolKind::Definition: return LLG_SLANG_SEMANTIC_DEFINITION;
    case SymbolKind::Instance:
    case SymbolKind::InstanceArray:
    case SymbolKind::CheckerInstance: return LLG_SLANG_SEMANTIC_INSTANCE;
    case SymbolKind::InstanceBody:
    case SymbolKind::CheckerInstanceBody:
    case SymbolKind::AnonymousProgram: return LLG_SLANG_SEMANTIC_SCOPE;
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
    case SymbolKind::ConstraintBlock: return LLG_SLANG_SEMANTIC_CONSTRAINT_BLOCK;
    case SymbolKind::CovergroupType:
    case SymbolKind::CovergroupBody:
    case SymbolKind::Coverpoint:
    case SymbolKind::CoverageBin:
    case SymbolKind::CoverCross: return LLG_SLANG_SEMANTIC_COVERAGE;
    case SymbolKind::RandSeqProduction: return LLG_SLANG_SEMANTIC_PRODUCTION;
    case SymbolKind::SpecifyBlock:
    case SymbolKind::TimingPath:
    case SymbolKind::PulseStyle:
    case SymbolKind::SystemTimingCheck: return LLG_SLANG_SEMANTIC_SPECIFY;
    default: return LLG_SLANG_SEMANTIC_UNSUPPORTED;
  }
}

// Repository codes for a module path's polarity and edge fields
// (LLG_SLANG_SPECIFY_PATH_*).
uint64_t specifyPolarityCode(TimingPathSymbol::Polarity polarity) {
  switch (polarity) {
    case TimingPathSymbol::Polarity::Unknown: return 0;
    case TimingPathSymbol::Polarity::Positive: return 1;
    case TimingPathSymbol::Polarity::Negative: return 2;
  }
  return 0;
}

uint64_t specifyEdgeCode(EdgeKind edge) {
  switch (edge) {
    case EdgeKind::None: return 0;
    case EdgeKind::PosEdge: return 1;
    case EdgeKind::NegEdge: return 2;
    case EdgeKind::BothEdges: return 3;
  }
  return 0;
}

uint64_t specifyPathAuxiliary(const TimingPathSymbol& path) {
  uint64_t auxiliary = 0;
  if (path.connectionKind == TimingPathSymbol::ConnectionKind::Full)
    auxiliary |= LLG_SLANG_SPECIFY_PATH_FULL;
  auxiliary |= specifyPolarityCode(path.polarity)
               << LLG_SLANG_SPECIFY_PATH_POLARITY_SHIFT;
  auxiliary |= specifyEdgeCode(path.edgeIdentifier)
               << LLG_SLANG_SPECIFY_PATH_EDGE_SHIFT;
  auxiliary |= specifyPolarityCode(path.edgePolarity)
               << LLG_SLANG_SPECIFY_PATH_EDGE_POLARITY_SHIFT;
  if (path.isStateDependent)
    auxiliary |= LLG_SLANG_SPECIFY_PATH_STATE_DEPENDENT;
  return auxiliary;
}

// A pulse-limit specparam (30.7.1). Slang marks only the two-value form
// (`isPathPulse`); `PATHPULSE$name = r;` declared in a specify block is a
// pulse limit as well, with the error limit equal to the reject limit.
bool isPulseLimitSpecparam(const SpecparamSymbol& symbol) {
  if (symbol.isPathPulse)
    return true;
  const Scope* parent = symbol.getParentScope();
  return symbol.name.starts_with("PATHPULSE$") && parent != nullptr &&
         parent->asSymbol().kind == SymbolKind::SpecifyBlock;
}

// The input (`source`) or output terminal named by a pulse-limit specparam
// `PATHPULSE$in$out`, or null for the module-wide `PATHPULSE$` and for a
// name that does not resolve to a value. Slang resolves the two-value form
// itself; the single-value form is resolved here the same way (first `$`
// after the prefix separates the names, lookup in the module scope) without
// diagnostics, since a pulse limit naming no path simply applies to none.
const Symbol* pulseLimitTerminal(const SpecparamSymbol& symbol, bool source) {
  if (symbol.isPathPulse)
    return source ? symbol.getPathSource() : symbol.getPathDest();
  constexpr std::string_view prefix = "PATHPULSE$";
  const std::string_view path = symbol.name.substr(prefix.size());
  const size_t index = path.find('$');
  if (index == std::string_view::npos || index == 0 || index + 1 == path.size())
    return nullptr;
  const Scope* block = symbol.getParentScope();
  const Scope* module = block == nullptr ? nullptr : block->asSymbol().getParentScope();
  if (module == nullptr)
    return nullptr;
  const std::string_view name = source ? path.substr(0, index) : path.substr(index + 1);
  const Symbol* terminal = Lookup::unqualified(*module, name);
  return terminal != nullptr && terminal->isValue() ? terminal : nullptr;
}

uint64_t specifyPulseStyleCode(PulseStyleKind kind) {
  switch (kind) {
    case PulseStyleKind::OnEvent: return LLG_SLANG_SPECIFY_PULSE_ONEVENT;
    case PulseStyleKind::OnDetect: return LLG_SLANG_SPECIFY_PULSE_ONDETECT;
    case PulseStyleKind::ShowCancelled: return LLG_SLANG_SPECIFY_PULSE_SHOWCANCELLED;
    case PulseStyleKind::NoShowCancelled: return LLG_SLANG_SPECIFY_PULSE_NOSHOWCANCELLED;
  }
  throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR, "unknown pulse style kind");
}

// An unknown timing check (an error the frontend has already reported)
// maps to 0, which the importer rejects only if the block is lowered.
uint64_t specifyTimingCheckCode(SystemTimingCheckKind kind) {
  switch (kind) {
    case SystemTimingCheckKind::Setup: return LLG_SLANG_SPECIFY_CHECK_SETUP;
    case SystemTimingCheckKind::Hold: return LLG_SLANG_SPECIFY_CHECK_HOLD;
    case SystemTimingCheckKind::SetupHold: return LLG_SLANG_SPECIFY_CHECK_SETUPHOLD;
    case SystemTimingCheckKind::Recovery: return LLG_SLANG_SPECIFY_CHECK_RECOVERY;
    case SystemTimingCheckKind::Removal: return LLG_SLANG_SPECIFY_CHECK_REMOVAL;
    case SystemTimingCheckKind::RecRem: return LLG_SLANG_SPECIFY_CHECK_RECREM;
    case SystemTimingCheckKind::Skew: return LLG_SLANG_SPECIFY_CHECK_SKEW;
    case SystemTimingCheckKind::TimeSkew: return LLG_SLANG_SPECIFY_CHECK_TIMESKEW;
    case SystemTimingCheckKind::FullSkew: return LLG_SLANG_SPECIFY_CHECK_FULLSKEW;
    case SystemTimingCheckKind::Period: return LLG_SLANG_SPECIFY_CHECK_PERIOD;
    case SystemTimingCheckKind::Width: return LLG_SLANG_SPECIFY_CHECK_WIDTH;
    case SystemTimingCheckKind::NoChange: return LLG_SLANG_SPECIFY_CHECK_NOCHANGE;
    case SystemTimingCheckKind::Unknown: return 0;
  }
  return 0;
}

// The formal argument kinds of each timing check, in signature order (IEEE
// 1800-2009 Syntax 31-3..31-14, the frontend's own table): events, limits,
// the notifier, and the $setuphold/$recrem timestamp/timecheck conditions and
// delayed reference/data signals.
enum class TimingArg : uint8_t { Event, Limit, Notifier, Condition, Delayed };

std::span<const TimingArg> timingCheckFormals(SystemTimingCheckKind kind) {
  using enum TimingArg;
  static constexpr TimingArg pair[] = {Event, Event, Limit, Notifier};
  static constexpr TimingArg combined[] = {Event, Event, Limit, Limit, Notifier,
                                           Condition, Condition, Delayed, Delayed};
  static constexpr TimingArg timeSkew[] = {Event, Event, Limit, Notifier, Limit, Limit};
  static constexpr TimingArg fullSkew[] = {Event, Event, Limit, Limit, Notifier,
                                           Limit, Limit};
  static constexpr TimingArg period[] = {Event, Limit, Notifier};
  static constexpr TimingArg width[] = {Event, Limit, Limit, Notifier};
  static constexpr TimingArg noChange[] = {Event, Event, Limit, Limit, Notifier};
  switch (kind) {
    case SystemTimingCheckKind::Setup:
    case SystemTimingCheckKind::Hold:
    case SystemTimingCheckKind::Recovery:
    case SystemTimingCheckKind::Removal:
    case SystemTimingCheckKind::Skew: return pair;
    case SystemTimingCheckKind::SetupHold:
    case SystemTimingCheckKind::RecRem: return combined;
    case SystemTimingCheckKind::TimeSkew: return timeSkew;
    case SystemTimingCheckKind::FullSkew: return fullSkew;
    case SystemTimingCheckKind::Period: return period;
    case SystemTimingCheckKind::Width: return width;
    case SystemTimingCheckKind::NoChange: return noChange;
    case SystemTimingCheckKind::Unknown: return {};
  }
  return {};
}

// The LLG_SLANG_SPECIFY_EDGE_* bit of one edge descriptor (31.5); z counts
// as x. Zero for a pair that is not a transition.
uint64_t specifyEdgeDescriptorBit(char from, char to) {
  auto state = [](char c) {
    switch (c) {
      case '0': return 0;
      case '1': return 1;
      case 'x': case 'X': case 'z': case 'Z': return 2;
      default: return 3;
    }
  };
  static constexpr uint64_t bits[3][3] = {
      {0, LLG_SLANG_SPECIFY_EDGE_01, LLG_SLANG_SPECIFY_EDGE_0X},
      {LLG_SLANG_SPECIFY_EDGE_10, 0, LLG_SLANG_SPECIFY_EDGE_1X},
      {LLG_SLANG_SPECIFY_EDGE_X0, LLG_SLANG_SPECIFY_EDGE_X1, 0}};
  const int f = state(from), t = state(to);
  return f < 3 && t < 3 ? bits[f][t] : 0;
}

// The transition mask of one timing check event (31.5): posedge and negedge
// are their edge-control shorthands, `edge` without a list is either edge.
uint64_t specifyTimingEventMask(const SystemTimingCheckSymbol::Arg& arg) {
  constexpr uint64_t rise =
      LLG_SLANG_SPECIFY_EDGE_01 | LLG_SLANG_SPECIFY_EDGE_0X | LLG_SLANG_SPECIFY_EDGE_X1;
  constexpr uint64_t fall =
      LLG_SLANG_SPECIFY_EDGE_10 | LLG_SLANG_SPECIFY_EDGE_1X | LLG_SLANG_SPECIFY_EDGE_X0;
  switch (arg.edge) {
    case EdgeKind::None: return 0;
    case EdgeKind::PosEdge: return rise;
    case EdgeKind::NegEdge: return fall;
    case EdgeKind::BothEdges: break;
  }
  if (arg.edgeDescriptors.empty())
    return rise | fall;
  uint64_t mask = 0;
  for (const auto& descriptor : arg.edgeDescriptors)
    mask |= specifyEdgeDescriptorBit(descriptor[0], descriptor[1]);
  return mask;
}

// TIMING_CHECK auxiliary (see LLG_SLANG_SPECIFY_CHECK_*): kind, the edge
// masks of the event arguments, and the actual argument count and empty
// positions from the syntax (the bound arguments drop both on an error).
uint64_t specifyTimingCheckAuxiliary(const SystemTimingCheckSymbol& check) {
  uint64_t auxiliary = specifyTimingCheckCode(check.timingCheckKind);
  const auto formals = timingCheckFormals(check.timingCheckKind);
  const auto args = check.getArguments();
  uint32_t event = 0;
  for (size_t index = 0; index < args.size() && index < formals.size(); index++) {
    if (formals[index] != TimingArg::Event)
      continue;
    if (args[index].expr && event < 2)
      auxiliary |= specifyTimingEventMask(args[index])
                   << LLG_SLANG_SPECIFY_CHECK_EVENT_SHIFT(event);
    event++;
  }
  if (const syntax::SyntaxNode* node = check.getSyntax();
      node && node->kind == syntax::SyntaxKind::SystemTimingCheck) {
    const auto& actuals = node->as<syntax::SystemTimingCheckSyntax>().args;
    const size_t count = std::min<size_t>(actuals.size(), 0xff);
    for (size_t index = 0; index < count && index < 16; index++) {
      if (actuals[index]->kind == syntax::SyntaxKind::EmptyTimingCheckArg)
        auxiliary |= uint64_t{1} << (LLG_SLANG_SPECIFY_CHECK_EMPTY_SHIFT + index);
    }
    auxiliary |= uint64_t{count} << LLG_SLANG_SPECIFY_CHECK_COUNT_SHIFT;
  }
  return auxiliary;
}

// Repository codes for Slang constraint kinds; 0 for an invalid constraint,
// which is exported as an unsupported node.
uint32_t semanticConstraintKind(ConstraintKind kind) {
  switch (kind) {
    case ConstraintKind::List: return LLG_SLANG_CONSTRAINT_LIST;
    case ConstraintKind::Expression: return LLG_SLANG_CONSTRAINT_EXPRESSION;
    case ConstraintKind::Implication: return LLG_SLANG_CONSTRAINT_IMPLICATION;
    case ConstraintKind::Conditional: return LLG_SLANG_CONSTRAINT_CONDITIONAL;
    case ConstraintKind::Uniqueness: return LLG_SLANG_CONSTRAINT_UNIQUENESS;
    case ConstraintKind::DisableSoft: return LLG_SLANG_CONSTRAINT_DISABLE_SOFT;
    case ConstraintKind::SolveBefore: return LLG_SLANG_CONSTRAINT_SOLVE_BEFORE;
    case ConstraintKind::Foreach: return LLG_SLANG_CONSTRAINT_FOREACH;
    case ConstraintKind::Invalid: return 0;
  }
  return 0;
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
    case StatementKind::RandCase: return LLG_SLANG_STMT_RANDCASE;
    case StatementKind::RandSequence: return LLG_SLANG_STMT_RANDSEQUENCE;
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

uint32_t semanticPatternKind(PatternKind kind) {
  switch (kind) {
    case PatternKind::Invalid: return LLG_SLANG_PATTERN_INVALID;
    case PatternKind::Wildcard: return LLG_SLANG_PATTERN_WILDCARD;
    case PatternKind::Constant: return LLG_SLANG_PATTERN_CONSTANT;
    case PatternKind::Variable: return LLG_SLANG_PATTERN_VARIABLE;
    case PatternKind::Tagged: return LLG_SLANG_PATTERN_TAGGED;
    case PatternKind::Structure: return LLG_SLANG_PATTERN_STRUCTURE;
  }
  return LLG_SLANG_PATTERN_INVALID;
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
    case ExpressionKind::CopyClass: return LLG_SLANG_EXPR_COPY_CLASS;
    case ExpressionKind::Dist: return LLG_SLANG_EXPR_DIST;
    case ExpressionKind::NewCovergroup: return LLG_SLANG_EXPR_NEW_COVERGROUP;
    case ExpressionKind::MinTypMax: return LLG_SLANG_EXPR_MIN_TYP_MAX;
    case ExpressionKind::ValueRange: return LLG_SLANG_EXPR_VALUE_RANGE;
    case ExpressionKind::AssertionInstance: return LLG_SLANG_EXPR_ASSERTION_INSTANCE;
    case ExpressionKind::ClockingEvent: return LLG_SLANG_EXPR_CLOCKING_EVENT;
    case ExpressionKind::TaggedUnion: return LLG_SLANG_EXPR_TAGGED_UNION;
    case ExpressionKind::LValueReference: return LLG_SLANG_EXPR_UPDATE_CURRENT;
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

std::optional<int32_t> literalIndex(const Expression& expression) {
  if (expression.kind == ExpressionKind::IntegerLiteral)
    return expression.as<IntegerLiteral>().getValue().as<int32_t>();
  if (const ConstantValue* value = expression.getConstant();
      value && value->isInteger())
    return value->integer().as<int32_t>();
  return std::nullopt;
}

// Instance-array connections (IEEE 1364-2001 7.1.5, 12.1.3.5; IEEE 1800-2009
// 23.3.3.5, 28.3.6) are sliced per element as `X[l:r][i]` or `X[l:r][m:n]`.
// Slang keeps the declared numbering in a constant slice's type, so constant
// selects inside that slice address exactly `X[i]` or `X[m:n]`. Expose that
// base so every consumer sees an ordinary select. An indexed slice with a
// runtime base is renumbered from the value type's bound and stays a slice.
const Expression& peelConstantSlices(const Expression& value, int32_t left,
                                     int32_t right) {
  const Expression* base = &value;
  while (base->kind == ExpressionKind::RangeSelect) {
    const auto& slice = base->as<RangeSelectExpression>();
    const Type& sliceType = *slice.type;
    const Type& valueType = *slice.value().type;
    if (!sliceType.hasFixedRange() || !valueType.hasFixedRange())
      break;
    const ConstantRange sliceRange = sliceType.getFixedRange();
    const ConstantRange valueRange = valueType.getFixedRange();
    if (!sliceRange.containsPoint(left) || !sliceRange.containsPoint(right) ||
        !valueRange.containsPoint(left) || !valueRange.containsPoint(right))
      break;
    if (slice.getSelectionKind() != RangeSelectionKind::Simple &&
        !literalIndex(slice.left()))
      break;
    base = &slice.value();
  }
  return *base;
}

const Expression& elementSelectBase(const ElementSelectExpression& expression) {
  const std::optional<int32_t> index = literalIndex(expression.selector());
  if (!index)
    return expression.value();
  return peelConstantSlices(expression.value(), *index, *index);
}

const Expression& rangeSelectBase(const RangeSelectExpression& expression) {
  if (expression.getSelectionKind() != RangeSelectionKind::Simple)
    return expression.value();
  const std::optional<int32_t> left = literalIndex(expression.left());
  const std::optional<int32_t> right = literalIndex(expression.right());
  if (!left || !right)
    return expression.value();
  return peelConstantSlices(expression.value(), *left, *right);
}

// An overloaded compound assignment or increment (IEEE 1800-2009 11.11) keeps
// its compound operator only while its target is bound once; its right side is
// then the bound function's call, never Slang's built-in binary expansion.
bool isOverloadedUpdate(const AssignmentExpression& expression) {
  if (!expression.op)
    return false;
  const Expression* right = unwrapImplicitConversions(expression.right());
  return right && right->kind == ExpressionKind::Call;
}

const Expression* compoundAssignmentSourceRhs(
    const AssignmentExpression& expression) {
  if (!expression.op || isOverloadedUpdate(expression))
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
    case TimingControlKind::EventList: return LLG_SLANG_TIMING_EVENT_LIST;
    case TimingControlKind::BlockEventList: return LLG_SLANG_TIMING_BLOCK_EVENT_LIST;
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

// Whether the compilation unit `unit` declares its time unit or precision
// (`timeunit`/`timeprecision` outside all other declarations). Slang otherwise
// reports the compilation default for it, which consumers must be able to tell
// apart: 3.14.2.3 sets the scale of `$unit` only through these declarations,
// never through a `timescale directive. The unit's syntax is borrowed from the
// live compilation.
bool compilationUnitDeclaresTimeScale(const CompilationUnitSymbol& unit) {
  const syntax::SyntaxNode* node = unit.getSyntax();
  if (!node || node->kind != syntax::SyntaxKind::CompilationUnit)
    return false;
  for (const syntax::MemberSyntax* member :
       node->as<syntax::CompilationUnitSyntax>().members) {
    if (member->kind == syntax::SyntaxKind::TimeUnitsDeclaration)
      return true;
  }
  return false;
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

  // Slang elaborates and looks up one definition per name in a scope; a
  // same-named definition in another file is elaborated only when an
  // explicit top selection makes it an unreferenced definition. Capture
  // every such body no instance reached as Slang captures an unreferenced
  // one (a nameless uninstantiated instance parented to the root, never a
  // top), so its names bind within the body that declares them. Called
  // after the root traversal, once diagnostics are taken: the shadowed
  // body adds none.
  void captureShadowedDefinitions(Compilation& compilation, const RootSymbol& root) {
    // Only compilation-unit definitions share the root namespace; Slang
    // rejects a nested duplicate before it becomes a definition.
    const auto rootDefinition = [](const Symbol& definition) {
      const Scope* scope = definition.getParentScope();
      return definition.kind == SymbolKind::Definition && scope &&
             (scope->asSymbol().kind == SymbolKind::CompilationUnit ||
              scope->asSymbol().kind == SymbolKind::Root);
    };
    const std::vector<const Symbol*> definitions = compilation.getDefinitions();
    std::unordered_map<std::string_view, size_t> namesakes;
    for (const Symbol* definition : definitions) {
      if (rootDefinition(*definition))
        ++namesakes[definition->name];
    }
    const uint64_t rootId = capture.ensureSemantic(&root);
    for (const Symbol* definition : definitions) {
      if (!rootDefinition(*definition) || namesakes[definition->name] < 2 ||
          sourceBodies.contains(definition->getSyntax()))
        continue;
      auto& instance = InstanceSymbol::createInvalid(
          compilation, definition->as<DefinitionSymbol>());
      instance.setParent(root);
      parents.push_back(rootId);
      instance.visit(*this);
      parents.pop_back();
    }
  }

  template<std::derived_from<Symbol> T>
  void handle(const T& symbol) {
    if constexpr (std::same_as<T, InstanceBodySymbol>) {
      // Every mode records the reached definition bodies so shadowed
      // duplicates are found (captureShadowedDefinitions); library-unit
      // capture keeps one body per definition.
      const bool firstBody =
          sourceBodies.insert(symbol.getDefinition().getSyntax()).second;
      if (capture.declarationOnly && !firstBody)
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
    // An implicitly instantiated nested module, interface or program (SV
    // 23.4, 24.3) uses its declaration as instance syntax. The definition and
    // the instance body own that declaration's identity; grouping the
    // instance with them would make the body's identity edge name its own
    // instance.
    bool implicitInstance = false;
    if constexpr (std::same_as<T, InstanceSymbol>) {
      implicitInstance = symbol.getSyntax() &&
                         syntax::ModuleDeclarationSyntax::isKind(symbol.getSyntax()->kind);
    }
    if (!implicitInstance)
      capture.sourceIdentity(symbol.getSyntax(), id);
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    result.kind = semanticSymbolKind(symbol.kind);
    // A production formal is an automatic variable of each generation of its
    // production (IEEE 1800-2009 18.17.7), bound by an ITEM's assignments.
    if constexpr (std::same_as<T, FormalArgumentSymbol>) {
      if (isProductionFormal(symbol))
        result.kind = LLG_SLANG_SEMANTIC_VARIABLE;
    }
    result.name = storeString(capture.output, symbol.name);
    result.detail = storeString(capture.output, toString(symbol.kind));
    result.range = symbol.location.valid()
        ? capture.span(symbol.location, symbol.name.size())
        : (symbol.getSyntax() ? capture.span(symbol.getSyntax()->sourceRange())
                              : LlgSlangSourceRange{LLG_SLANG_INVALID_ID, 0, 0});
    // A name written in a macro argument: the node spans the macro use site,
    // so the written name token is bound to the declaration here.
    // A name copied from a `define body binds the body text the same way.
    if (symbol.kind != SymbolKind::TransparentMember) {
      if (auto written = capture.macroArgumentSpan(symbol.location, symbol.name))
        capture.lexicalBinding(*written, id, LLG_SLANG_LEXICAL_ROLE_DECLARATION);
      else if (auto body = capture.macroBodySpan(symbol.location, symbol.name))
        capture.lexicalBinding(*body, id, LLG_SLANG_LEXICAL_ROLE_DECLARATION,
                               LLG_SLANG_LEXICAL_UNKNOWN, true);
    }
    if constexpr (std::same_as<T, SequenceSymbol> || std::same_as<T, PropertySymbol> ||
                  std::same_as<T, LetDeclSymbol>)
      bindAssertionDeclarationNames(symbol, symbol);
    if constexpr (std::derived_from<T, ValueSymbol>) {
      result.type_id = capture.type(symbol.getType());
      capture.typedefEdge(id, symbol.getType());
      const Scope* parent = symbol.getParentScope();
      if (parent && parent->asSymbol().kind == SymbolKind::CompilationUnit)
        capture.unitValueDeclarations.emplace(id, symbol.location);
    }
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
    if constexpr (std::same_as<T, TypeAliasType>) {
      const Type& target = symbol.targetType.getType();
      result.type_id = capture.type(target);
      capture.typedefEdge(id, target);
      switch (target.kind) {
        case SymbolKind::ScalarType:
        case SymbolKind::PredefinedIntegerType:
        case SymbolKind::FloatingType:
        case SymbolKind::StringType:
        case SymbolKind::CHandleType:
        case SymbolKind::EventType:
          result.auxiliary |= LLG_SLANG_TYPEDEF_BUILTIN;
          break;
        default:
          break;
      }
    }
    if constexpr (std::same_as<T, EnumValueSymbol>)
      result.constant_id = capture.constant(symbol.getValue());
    // An unresolved instance keeps the definition name it was written with
    // so editor projections can describe the black box without syntax.
    if constexpr (std::same_as<T, UninstantiatedDefSymbol>)
      result.definition_name = storeString(capture.output, symbol.definitionName);
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
    if constexpr (std::same_as<T, InstanceArraySymbol>) {
      result.subkind = LLG_SLANG_INSTANCE_ARRAY;
      // The declared [left:right] of this array level (ABI v54), for the
      // VPI instance array objects (IEEE 1800-2009 37.11).
      result.auxiliary = uint64_t(uint32_t(symbol.range.left)) |
                         (uint64_t(uint32_t(symbol.range.right)) << 32);
    }
    if constexpr (std::same_as<T, InstanceBodySymbol> ||
                  std::same_as<T, CheckerInstanceBodySymbol>)
      result.subkind = LLG_SLANG_INSTANCE_BODY;
    if constexpr (std::same_as<T, CheckerInstanceSymbol>)
      describeCheckerInstance(result, symbol);
    if constexpr (std::same_as<T, GenerateBlockSymbol>) {
      result.subkind = LLG_SLANG_GENERATE_BLOCK;
      std::string externalName = symbol.getExternalName();
      if (symbol.isUninstantiated)
        result.flags |= LLG_SLANG_SEMANTIC_UNINSTANTIATED;
      if (const SVInt* arrayIndex = symbol.getArrayIndex()) {
        const Scope* parentScope = symbol.getParentScope();
        if (!parentScope ||
            parentScope->asSymbol().kind != SymbolKind::GenerateBlockArray)
          throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                              "loop generate block has no array parent");
        externalName = parentScope->asSymbol()
                           .as<GenerateBlockArraySymbol>()
                           .getExternalName();
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
      const Scope* parentScope = symbol.getParentScope();
      if (parentScope &&
          parentScope->asSymbol().kind != SymbolKind::CompilationUnit &&
          parentScope->asSymbol().kind != SymbolKind::Root)
        result.flags |= LLG_SLANG_SEMANTIC_LOCAL;
      addTimeScale(result, symbol.timeScale);
      if (symbol.unconnectedDrive == UnconnectedDrive::Pull0)
        result.auxiliary |= LLG_SLANG_UNCONNECTED_PULL0;
      else if (symbol.unconnectedDrive == UnconnectedDrive::Pull1)
        result.auxiliary |= LLG_SLANG_UNCONNECTED_PULL1;
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
    if constexpr (std::same_as<T, PackageSymbol> ||
                  std::same_as<T, CompilationUnitSymbol>)
      capture.sourceLibrary(id, symbol.getSourceLibrary());
    if constexpr (std::same_as<T, InstanceSymbol>) {
      // The configuration controlling the binding (IEEE 1800-2009 33.7).
      std::string config;
      if (symbol.resolvedConfig) {
        const ConfigBlockSymbol& block = symbol.resolvedConfig->useConfig;
        if (const SourceLibrary* library = block.getSourceLibrary())
          config = library->name + "." + std::string(block.name);
        else
          config = std::string(block.name);
      }
      capture.sourceLibrary(id, symbol.getSourceLibrary(), config);
      const DefinitionSymbol& definition = symbol.getDefinition();
      const auto drive = definition.unconnectedDrive;
      if (drive == UnconnectedDrive::Pull0)
        result.auxiliary |= LLG_SLANG_UNCONNECTED_PULL0;
      else if (drive == UnconnectedDrive::Pull1)
        result.auxiliary |= LLG_SLANG_UNCONNECTED_PULL1;
      if (definition.cellDefine)
        result.auxiliary |= LLG_SLANG_INSTANCE_CELL_DEFINE;
      if (definition.defaultLifetime == VariableLifetime::Automatic)
        result.auxiliary |= LLG_SLANG_INSTANCE_AUTOMATIC;
      result.auxiliary |=
          (uint64_t(semanticNetKind(definition.defaultNetType.netKind)) &
           LLG_SLANG_INSTANCE_NET_TYPE_MASK)
          << LLG_SLANG_INSTANCE_NET_TYPE_SHIFT;
    }
    if constexpr (std::same_as<T, PackageSymbol>)
      addTimeScale(result, symbol.timeScale);
    // An undeclared `$unit` scale is left unset rather than reporting the
    // compilation default as if it were declared.
    if constexpr (std::same_as<T, CompilationUnitSymbol>) {
      if (compilationUnitDeclaresTimeScale(symbol))
        addTimeScale(result, symbol.timeScale);
    }
    if constexpr (std::same_as<T, NetSymbol>) {
      result.subkind = semanticNetKind(symbol.netType.netKind);
      if (symbol.isImplicit)
        result.flags |= LLG_SLANG_SEMANTIC_IMPLICIT;
      if (symbol.expansionHint == NetSymbol::Vectored)
        result.auxiliary |= LLG_SLANG_NET_VECTORED;
      else if (symbol.expansionHint == NetSymbol::Scalared)
        result.auxiliary |= LLG_SLANG_NET_SCALARED;
      addDriveStrength(result, symbol.getDriveStrength());
    }
    if constexpr (std::same_as<T, PrimitiveSymbol>) {
      result.subkind = semanticPrimitiveType(symbol);
      result.flags |= LLG_SLANG_SEMANTIC_PRIMITIVE_DECLARATION;
      capture.captureUdpTable(id, symbol);
    }
    if constexpr (std::same_as<T, PrimitiveInstanceSymbol>) {
      // `ensureSemantic` may grow semantic_nodes and invalidate references
      // into that vector. Resolve the primitive declaration before borrowing
      // the instance record so the instance flags and target remain owned by
      // the live vector element.
      const uint64_t primitiveId =
          capture.ensureSemantic(&symbol.primitiveType);
      auto& result =
          capture.output.semantic_nodes[static_cast<size_t>(id)];
      result.subkind = semanticPrimitiveType(symbol.primitiveType);
      result.definition_name = storeString(capture.output,
                                           symbol.primitiveType.name);
      // Keep the declaration identity beside the display name.  Primitive
      // names are looked up in their owning scope, so a name-only join can
      // select a different UDP table when declarations share a spelling.
      result.target_id = primitiveId;
      capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, primitiveId);
      result.flags |= LLG_SLANG_SEMANTIC_PRIMITIVE_INSTANCE;
      addDriveStrength(result, symbol.getDriveStrength());
      if (!symbol.arrayPath.empty()) {
        // An instance array element (ABI v41): which slice of a connection
        // as wide as the whole array is its own (IEEE 1800-2009 28.3.6).
        // Slang slices only input connections itself; this exports the same
        // position for the bidirectional terminals it binds whole.
        SmallVector<ConstantRange, 8> dimensions;
        symbol.getArrayDimensions(dimensions);
        if (dimensions.size() != symbol.arrayPath.size())
          throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                              "primitive array path and dimensions disagree");
        uint64_t offset = 0;
        uint64_t count = 1;
        for (size_t i = 0; i < dimensions.size(); ++i) {
          const uint64_t width = dimensions[i].width();
          uint64_t index = symbol.arrayPath[i];
          if (index >= width)
            throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                                "primitive array position is outside its dimension");
          // arrayPath counts from the lower bound; the right-hand index
          // takes the least significant slice.
          if (!dimensions[i].isDescending())
            index = width - index - 1;
          offset = offset * width + index;
          count *= width;
          if (count > UINT32_MAX) break;
        }
        if (count <= UINT32_MAX)
          result.auxiliary = (count << 32) | offset;
      }
    }
    if constexpr (std::same_as<T, PrimitivePortSymbol>) {
      result.flags |= LLG_SLANG_SEMANTIC_PRIMITIVE_PORT;
      addDirection(result, symbol.direction);
    }
    if constexpr (std::same_as<T, PortSymbol>)
      addDirection(result, symbol.direction);
    if constexpr (std::same_as<T, MultiPortSymbol>)
      addDirection(result, symbol.direction);
    if constexpr (std::same_as<T, PortSymbol> ||
                  std::same_as<T, MultiPortSymbol>) {
      // A `.name(expression)` port keeps the expression's syntax (whose
      // parent is the explicit port) or, without an expression, its own.
      const syntax::SyntaxNode* portSyntax = symbol.getSyntax();
      auto explicitPort = [](const syntax::SyntaxNode* node) {
        return node && (node->kind == syntax::SyntaxKind::ExplicitAnsiPort ||
                        node->kind == syntax::SyntaxKind::ExplicitNonAnsiPort);
      };
      if (portSyntax &&
          (explicitPort(portSyntax) || explicitPort(portSyntax->parent)))
        result.auxiliary |= LLG_SLANG_PORT_EXPLICIT_NAME;
    }
    if constexpr (std::same_as<T, PortSymbol>) {
      // A non-ANSI port is located at its io declaration; its place in the
      // module header is the external location.
      const SourceLocation header = symbol.externalLoc;
      if (!symbol.isAnsiPort && header.valid() && symbol.location.valid() &&
          header.buffer() == symbol.location.buffer() &&
          header.offset() != symbol.location.offset() &&
          header.offset() < UINT32_MAX)
        result.auxiliary |= (uint64_t(header.offset()) + 1u) << 32;
    }
    if constexpr (std::same_as<T, ModportPortSymbol>)
      addDirection(result, symbol.direction);
    if constexpr (std::same_as<T, FormalArgumentSymbol>) {
      if (!isProductionFormal(symbol))
        addDirection(result, symbol.direction);
      if (symbol.flags.has(VariableFlags::Const))
        result.auxiliary |= LLG_SLANG_ARGUMENT_CONST_REF;
      if (symbol.flags.has(VariableFlags::RefStatic))
        result.auxiliary |= LLG_SLANG_ARGUMENT_REF_STATIC;
      if (symbol.flags.has(VariableFlags::CoverageSampleFormal))
        result.auxiliary |= LLG_SLANG_ARGUMENT_COVERAGE_SAMPLE;
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
    if constexpr (std::same_as<T, AnonymousProgramSymbol>)
      result.subkind = LLG_SLANG_SCOPE_ANONYMOUS_PROGRAM;
    // Specify items have no name, so they are located by their syntax.
    if constexpr (std::same_as<T, SpecifyBlockSymbol> ||
                  std::same_as<T, TimingPathSymbol> ||
                  std::same_as<T, PulseStyleSymbol> ||
                  std::same_as<T, SystemTimingCheckSymbol>) {
      if (const syntax::SyntaxNode* syntaxNode = symbol.getSyntax())
        result.range = capture.span(syntaxNode->sourceRange());
    }
    if constexpr (std::same_as<T, SpecifyBlockSymbol>)
      result.subkind = LLG_SLANG_SPECIFY_BLOCK;
    if constexpr (std::same_as<T, TimingPathSymbol>) {
      result.subkind = LLG_SLANG_SPECIFY_PATH;
      result.auxiliary = specifyPathAuxiliary(symbol);
    }
    if constexpr (std::same_as<T, PulseStyleSymbol>) {
      result.subkind = LLG_SLANG_SPECIFY_PULSE_STYLE;
      result.auxiliary = specifyPulseStyleCode(symbol.pulseStyleKind);
    }
    if constexpr (std::same_as<T, SystemTimingCheckSymbol>) {
      result.subkind = LLG_SLANG_SPECIFY_TIMING_CHECK;
      result.auxiliary = specifyTimingCheckAuxiliary(symbol);
    }
    if constexpr (std::same_as<T, SpecparamSymbol>) {
      if (symbol.isPathPulse) {
        result.subkind = LLG_SLANG_PARAMETER_PATHPULSE;
        const ConstantValue& reject = symbol.getPulseRejectLimit();
        if (!reject.bad())
          result.constant_id = capture.constant(reject);
        const ConstantValue& error = symbol.getPulseErrorLimit();
        if (!error.bad())
          result.auxiliary = capture.constant(error) + 1;
      }
      else if (isPulseLimitSpecparam(symbol)) {
        result.subkind = LLG_SLANG_PARAMETER_PATHPULSE;
        const ConstantValue& reject = symbol.getValue();
        if (!reject.bad())
          result.constant_id = capture.constant(reject);
      }
      else {
        result.subkind = LLG_SLANG_PARAMETER_SPECPARAM;
        result.constant_id = capture.constant(symbol.getValue());
      }
    }
    if constexpr (std::same_as<T, RandSeqProductionSymbol>) {
      result.subkind = LLG_SLANG_PRODUCTION_DECLARATION;
      result.type_id = capture.type(symbol.getReturnType());
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
        if (symbol.flags.has(VariableFlags::Const))
          result.auxiliary |= LLG_SLANG_VARIABLE_CONST;
        // A free checker variable (IEEE 1800-2009 17.7).
        if (symbol.flags.has(VariableFlags::CheckerFreeVariable))
          result.auxiliary |= LLG_SLANG_VARIABLE_RAND;
        if constexpr (std::same_as<T, ClassPropertySymbol>) {
          if (symbol.randMode == RandMode::Rand)
            result.auxiliary |= LLG_SLANG_VARIABLE_RAND;
          else if (symbol.randMode == RandMode::RandC)
            result.auxiliary |= LLG_SLANG_VARIABLE_RANDC;
          if (symbol.visibility == Visibility::Local)
            result.auxiliary |= LLG_SLANG_VARIABLE_LOCAL;
          else if (symbol.visibility == Visibility::Protected)
            result.auxiliary |= LLG_SLANG_VARIABLE_PROTECTED;
        }
      }
    }
    if constexpr (std::same_as<T, ConstraintBlockSymbol>) {
      if (symbol.flags.has(ConstraintBlockFlags::Static))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_STATIC;
      if (symbol.flags.has(ConstraintBlockFlags::Pure))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_PURE;
      if (symbol.flags.has(ConstraintBlockFlags::Extern))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_EXTERN;
      if (symbol.flags.has(ConstraintBlockFlags::ExplicitExtern))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_EXPLICIT_EXTERN;
      if (symbol.flags.has(ConstraintBlockFlags::Initial))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_INITIAL;
      if (symbol.flags.has(ConstraintBlockFlags::Extends))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_EXTENDS;
      if (symbol.flags.has(ConstraintBlockFlags::Final))
        result.auxiliary |= LLG_SLANG_CONSTRAINT_BLOCK_FINAL;
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
      if (symbol.visibility == Visibility::Local)
        result.auxiliary |= LLG_SLANG_SUBROUTINE_LOCAL;
      else if (symbol.visibility == Visibility::Protected)
        result.auxiliary |= LLG_SLANG_SUBROUTINE_PROTECTED;
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
      if (const auto exported = dpiExportName(capture, symbol)) {
        // An export is never an import (the frontend rejects that), so the
        // linkage-name field carries the export's C identifier.
        result.auxiliary |= LLG_SLANG_SUBROUTINE_DPI_EXPORT;
        result.definition_name = storeString(capture.output, *exported);
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
    if constexpr (std::same_as<T, MethodPrototypeSymbol>) {
      // A class method prototype (`pure virtual` or `extern`, IEEE 1800-2009
      // 8.21, 8.24): the declaration the class holds for the method.
      if (symbol.subroutineKind == SubroutineKind::Task)
        result.flags |= LLG_SLANG_SEMANTIC_TASK;
      result.type_id = capture.type(symbol.getReturnType());
      if (symbol.flags.has(MethodFlags::Pure))
        result.auxiliary |= LLG_SLANG_SUBROUTINE_PURE;
      if (symbol.flags.has(MethodFlags::Static))
        result.auxiliary |= LLG_SLANG_SUBROUTINE_STATIC;
      if (symbol.isVirtual())
        result.auxiliary |= LLG_SLANG_SUBROUTINE_PROTOTYPE_VIRTUAL;
      if (symbol.visibility == Visibility::Local)
        result.auxiliary |= LLG_SLANG_SUBROUTINE_LOCAL;
      else if (symbol.visibility == Visibility::Protected)
        result.auxiliary |= LLG_SLANG_SUBROUTINE_PROTECTED;
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

    if constexpr (std::same_as<T, CovergroupType> ||
                  std::same_as<T, CovergroupBodySymbol> ||
                  std::same_as<T, CoverpointSymbol> ||
                  std::same_as<T, CoverageBinSymbol> ||
                  std::same_as<T, CoverCrossSymbol>)
      captureCoverageSymbol(id, symbol);
    parents.push_back(id);
    if constexpr (std::same_as<T, InstanceBodySymbol> ||
                  std::same_as<T, CheckerInstanceBodySymbol> ||
                  std::same_as<T, GenerateBlockSymbol>)
      disableScopes.push_back(&symbol);
    if constexpr (std::same_as<T, CheckerInstanceSymbol>) {
      if (!capture.declarationOnly)
        pushCheckerActuals(id, symbol);
    }
    // Slang's visitor does not descend into a modport port expression
    // `.p(expr)`. Capture it under the port so the low-connection role
    // below names a fully captured expression rather than a placeholder.
    if constexpr (std::same_as<T, ModportPortSymbol>) {
      if (const Expression* connection = symbol.getConnectionExpr())
        connection->visit(*this);
    }
    // Likewise a port whose port expression is a select (`a[1:0]`, IEEE
    // 1800-2009 23.2.2.1) or an explicit ANSI expression (23.2.2.2): capture
    // the internal expression under the port for its low-connection role.
    if constexpr (std::same_as<T, PortSymbol>) {
      if (const Expression* internal = symbol.getInternalExpr())
        internal->visit(*this);
    }
    if constexpr (std::same_as<T, UninstantiatedDefSymbol>) {
      for (const auto* parameter : symbol.paramExpressions)
        parameter->visit(*this);
      for (const auto* connection : symbol.getPortConnections()) {
        connection->visit(*this);
        // Slang binds an unknown definition's port expression against the
        // error type, which wraps the correctly bound expression in an
        // invalid conversion whose child its visitor does not descend into.
        // Capture that child under the black box as well so owned consumers
        // see the signals each connection names. Declaration-only capture
        // already binds these names from syntax.
        if (capture.declarationOnly || connection->kind != AssertionExprKind::Simple)
          continue;
        const Expression& actual =
            connection->template as<SimpleAssertionExpr>().expr;
        if (actual.kind != ExpressionKind::Invalid)
          continue;
        const Expression* child = actual.as<InvalidExpression>().child;
        if (child && !child->bad())
          child->visit(*this);
      }
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
    // An enum without a typedef has no alias to walk; its values appear in the
    // declaring scope only as transparent members. Capture each value so its
    // declaration token is bound even when nothing references it.
    if constexpr (std::same_as<T, TransparentMemberSymbol>) {
      if (symbol.wrapped.kind == SymbolKind::EnumValue)
        handle(symbol.wrapped.template as<EnumValueSymbol>());
    }
    if constexpr (std::same_as<T, ExplicitImportSymbol> ||
                  std::same_as<T, WildcardImportSymbol>)
      bindImportItem(symbol);
    if constexpr (std::same_as<T, NetAliasSymbol>) {
      uint32_t index = 0;
      for (const Expression* expression : symbol.getNetReferences())
        capture.semanticRole(id, expression, LLG_SLANG_EDGE_ALIAS_NET, index++);
    }
    if constexpr (std::same_as<T, RandSeqProductionSymbol>) {
      if (capture.declarationOnly)
        visitDefault(symbol);
      else
        captureProduction(id, symbol);
    }
    else {
      const Symbol* outer = std::exchange(lookupOwner, &symbol);
      visitDefault(symbol);
      lookupOwner = outer;
    }
    if constexpr (std::same_as<T, TimingPathSymbol>) {
      if (!capture.declarationOnly)
        captureTimingPath(id, symbol);
    }
    if constexpr (std::same_as<T, SystemTimingCheckSymbol>) {
      if (!capture.declarationOnly)
        captureTimingCheck(id, symbol);
    }
    if constexpr (std::same_as<T, PulseStyleSymbol>) {
      if (!capture.declarationOnly) {
        uint32_t index = 0;
        for (const Expression* terminal : symbol.getTerminals())
          specifyOperand(id, *terminal, LLG_SLANG_EDGE_OPERAND, index++);
      }
    }
    // The terminals a `PATHPULSE$in$out` name resolves to (30.7.1).
    if constexpr (std::same_as<T, SpecparamSymbol>) {
      if (!capture.declarationOnly && isPulseLimitSpecparam(symbol)) {
        if (const Symbol* source = pulseLimitTerminal(symbol, true))
          capture.semanticRole(id, source, LLG_SLANG_EDGE_OPERAND);
        if (const Symbol* dest = pulseLimitTerminal(symbol, false))
          capture.semanticRole(id, dest, LLG_SLANG_EDGE_LHS);
      }
    }
    if constexpr (std::same_as<T, CovergroupType> ||
                  std::same_as<T, CoverpointSymbol> ||
                  std::same_as<T, CoverageBinSymbol> ||
                  std::same_as<T, CoverCrossSymbol>)
      captureCoverageRoles(id, symbol);
    // A prototype without an out-of-block body, and a pure constraint, bind
    // to an invalid constraint: such a block has no BODY (18.5.1, 18.5.2).
    if constexpr (std::same_as<T, ConstraintBlockSymbol>) {
      const Constraint& body = symbol.getConstraints();
      if (!capture.declarationOnly && body.kind != ConstraintKind::Invalid)
        capture.semanticRole(id, &body, LLG_SLANG_EDGE_BODY);
    }
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
    if constexpr (std::same_as<T, InstanceBodySymbol> ||
                  std::same_as<T, CheckerInstanceBodySymbol> ||
                  std::same_as<T, GenerateBlockSymbol>)
      disableScopes.pop_back();
    if constexpr (std::same_as<T, CheckerInstanceSymbol>) {
      if (!capture.declarationOnly) {
        checkerActuals.pop_back();
        // The `default disable iff` condition at the instantiation, which a
        // `$inferred_disable` formal default stands for (16.15.7, 17.4).
        if (!disableScopes.empty()) {
          const Scope& scope = *disableScopes.back();
          if (const Expression* disable = scope.getCompilation().getDefaultDisable(scope)) {
            disable->visit(*this);
            capture.semanticRole(id, disable, LLG_SLANG_EDGE_DEFAULT_DISABLE);
          }
        }
      }
    }
    parents.pop_back();
    // The component ports of a concatenated port expression (`.p({a, b})`,
    // IEEE 1800-2009 23.2.2.1) are not scope members, but each one carries
    // its own direction, internal connection and (Slang splits the actual)
    // instance connection. Capture them beside the multi-port.
    if constexpr (std::same_as<T, MultiPortSymbol>) {
      for (const PortSymbol* component : symbol.ports)
        handle(*component);
    }
    captureSourceReferences(symbol);
    if constexpr (std::same_as<T, InterfacePortSymbol>)
      bindInterfacePortType(symbol);
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
    // Built-in compound assignments use an internal LValueReferenceExpression
    // as the left operand of Slang's expanded binary RHS, which capture never
    // visits (see compoundAssignmentSourceRhs). The only visited placeholder
    // is an overloaded update's current target value (UPDATE_CURRENT).

    if (capture.declarationOnly) {
      bindValueName(expression);
      bindNameTokens(expression);
      if constexpr (std::same_as<T, InvalidExpression>)
        bindInvalidSubtree(expression.child);
      visitDefault(expression);
      return;
    }
    const uint64_t id = capture.ensureSemantic(&expression);
    attach(id);
    if (!markVisited(id))
      return;
    // A let expansion inside a checker resolves its formals to the actual
    // arguments of this invocation while the expansion is captured.
    const bool inChecker = !checkerActuals.empty();
    const size_t letAliases = inChecker ? pushLetAliases(expression) : 0;
    if (inChecker)
      captureSyntax.push_back(expression.syntax ? expression.syntax
                              : captureSyntax.empty() ? nullptr
                                                      : captureSyntax.back());
    struct AliasGuard {
      std::vector<std::unordered_map<const syntax::SyntaxNode*, const syntax::SyntaxNode*>>&
          frames;
      size_t count;
      std::vector<const syntax::SyntaxNode*>* syntaxStack;
      ~AliasGuard() {
        frames.resize(frames.size() - count);
        if (syntaxStack)
          syntaxStack->pop_back();
      }
    } aliasGuard{assertionAliases, letAliases, inChecker ? &captureSyntax : nullptr};
    if (!checkerActuals.empty())
      markCheckerActual(id, expression);
    bindNameTokens(expression);
    bindValueNameToken(expression, true);
    if constexpr (std::same_as<T, InvalidExpression>)
      bindInvalidSubtree(expression.child);
    // Capturing a call's iterator or referenced symbol can append to the
    // semantic-node vector. Assemble a value and publish it by ID after those
    // insertions; a reference here would dangle when the vector grows.
    auto result = capture.output.semantic_nodes[static_cast<size_t>(id)];
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
      if (isOverloadedUpdate(expression))
        result.operation = expression.isOverloadedPostfix
            ? LLG_SLANG_OP_OVERLOAD_POST_UPDATE
            : LLG_SLANG_OP_OVERLOAD_UPDATE;
      else
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
      else
        // A cast to a typedef (`nib_t'(x)`) names it (37.55 detail 3).
        capture.typedefEdge(id, *expression.type);
      if (expression.conversionKind == ConversionKind::Propagated)
        result.flags |= LLG_SLANG_SEMANTIC_PROPAGATED_CONVERSION;
      if (expression.isConstCast)
        result.auxiliary |= LLG_SLANG_CONVERSION_CONST;
    }
    if constexpr (std::same_as<T, RangeSelectExpression>) {
      if (expression.getSelectionKind() == RangeSelectionKind::IndexedUp)
        result.flags |= LLG_SLANG_SEMANTIC_INDEXED_UP;
      else if (expression.getSelectionKind() == RangeSelectionKind::IndexedDown)
        result.flags |= LLG_SLANG_SEMANTIC_INDEXED_DOWN;
    }
    if constexpr (std::same_as<T, MemberAccessExpression>)
      result.name = storeString(capture.output, expression.member.name);
    if constexpr (std::same_as<T, TaggedUnionExpression>)
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
    capture.output.semantic_nodes[static_cast<size_t>(id)] = result;
    if (const Symbol* target = expression.getSymbolReference()) {
      const uint64_t targetId = describeReferenceTarget(*target);
      capture.output.semantic_nodes[static_cast<size_t>(id)].target_id = targetId;
      capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, targetId);
    }
    captureTypeNameReference(expression);
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
      // Inside a checker, a formal of this let, sequence or property that
      // a checker formal is passed to expands to that checker formal.
      const bool aliased = !checkerActuals.empty() && pushAssertionAliases(expression);
      expression.body.visit(*this);
      if (aliased)
        assertionAliases.pop_back();
      for (const auto& [formal, actual] : expression.arguments) {
        (void)formal;
        std::visit([this](auto* value) {
          if (value)
            value->visit(*this);
        }, actual);
      }
      // Declaration assignments of the instance's own local variables
      // (IEEE 1800-2009 16.10) are owned by each local, like any variable
      // initializer. Local formals take their initial value from the
      // bound actual instead, which the arguments above already carry.
      for (const LocalAssertionVarSymbol* local : expression.localVars) {
        const Expression* initializer = local ? local->getInitializer() : nullptr;
        if (!initializer || local->formalPort)
          continue;
        const uint64_t localId = captureReferenceTarget(*local);
        parents.push_back(localId);
        initializer->visit(*this);
        parents.pop_back();
        capture.semanticRole(localId, initializer, LLG_SLANG_EDGE_INITIALIZER);
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
    else if constexpr (std::same_as<T, StructuredAssignmentPatternExpression>) {
      if (useResolvedPatternElements(expression)) {
        // Slang has already resolved these fixed-array patterns to
        // declaration-order values. Ordinary explicit index-key patterns
        // remain on the semantic-key capture path.
        for (const Expression* element : expression.elements())
          element->visit(*this);
      }
      else {
        visitDefault(expression);
      }
    }
    else {
      visitDefault(expression);
    }
    parents.pop_back();
    addExpressionRoles(expression, id);
    capture.replaceChildRoles(id, LLG_SLANG_EDGE_OPERAND);
  }

  // The statements of a block with its own scope (named, or declaring
  // variables) look names up in that scope: the visitor reaches them through
  // the block statement, not through its block symbol.
  template<std::derived_from<Statement> T>
  void visitStatementChildren(const T& statement) {
    const Symbol* outer = lookupOwner;
    if constexpr (std::same_as<T, BlockStatement>) {
      if (statement.blockSymbol)
        lookupOwner = statement.blockSymbol;
    }
    visitDefault(statement);
    lookupOwner = outer;
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
      bindStatementNames(statement);
      if constexpr (std::same_as<T, InvalidStatement>)
        bindInvalidSubtree(statement.child);
      visitStatementChildren(statement);
      return;
    }
    const uint64_t id = capture.ensureSemantic(&statement);
    attach(id);
    if (!markVisited(id))
      return;
    bindStatementNames(statement);
    if constexpr (std::same_as<T, InvalidStatement>)
      bindInvalidSubtree(statement.child);
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    result.kind = LLG_SLANG_SEMANTIC_STATEMENT;
    result.subkind = semanticStatementKind(statement.kind);
    result.detail = storeString(capture.output, toString(statement.kind));
    result.range = capture.span(statement.sourceRange);
    if (statement.bad())
      result.flags |= LLG_SLANG_SEMANTIC_BAD;
    if constexpr (std::same_as<T, BlockStatement>) {
      // A block holding one statement spans only that statement; keep
      // where its `begin` or `fork` keyword is written.
      if (statement.syntax &&
          syntax::BlockStatementSyntax::isKind(statement.syntax->kind))
        result.auxiliary = capture.keywordOffset(
            statement.syntax->template as<syntax::BlockStatementSyntax>().begin.location(),
            result.range.file_id);
    }
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
    if constexpr (std::same_as<T, PatternCaseStatement>) {
      // Pattern cases carry the same case-condition and qualifier metadata
      // as ordinary cases.  Keep this on the owned node so matching is
      // selected from typed frontend data rather than reconstructed from
      // source text.
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
          result.subkind = LLG_SLANG_STMT_CONCURRENT_COVER;
          break;
        case AssertionKind::CoverSequence:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_COVER_SEQUENCE;
          break;
        case AssertionKind::Expect:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_EXPECT;
          break;
        case AssertionKind::Restrict:
          result.subkind = LLG_SLANG_STMT_CONCURRENT_RESTRICT;
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
    if constexpr (std::same_as<T, ProceduralCheckerStatement>) {
      // `result` may move once ensureSemantic grows the node table.
      result.subkind = LLG_SLANG_STMT_PROCEDURAL_CHECKER;
      uint32_t ordinal = 0;
      for (const Symbol* instance : statement.instances)
        proceduralCheckerEdges(id, *instance, ordinal);
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
    visitStatementChildren(statement);
    if constexpr (std::same_as<T, ConcurrentAssertionStatement>) {
      if (const Expression* disable = defaultDisable(statement))
        disable->visit(*this);
    }
    if constexpr (std::same_as<T, RandSequenceStatement>)
      captureRandSequence(id, statement);
    parents.pop_back();
    addStatementRoles(statement, id);
  }

  // The checker instances a procedural checker statement instantiates, array
  // elements in element order. Slang resolves them by name in the statement's
  // scope, which also holds them as members, so each is captured there.
  void proceduralCheckerEdges(uint64_t id, const Symbol& symbol, uint32_t& ordinal) {
    if (symbol.kind == SymbolKind::InstanceArray) {
      for (const Symbol* element : symbol.as<InstanceArraySymbol>().elements)
        proceduralCheckerEdges(id, *element, ordinal);
      return;
    }
    if (symbol.kind != SymbolKind::CheckerInstance)
      return;
    capture.semanticEdge(id, LLG_SLANG_EDGE_PROCEDURAL_CHECKER,
                         capture.ensureSemantic(&symbol), ordinal++);
  }

  // The `default disable iff` condition inferred for a concurrent assertion
  // (IEEE 1800-2009 16.16 b)): Slang resolves it through the enclosing
  // generate blocks, instance body and enclosing declarations but keeps it
  // only in its compilation. An expect statement is not a concurrent
  // assertion statement (16.15 vs. 16.18) and infers none.
  const Expression* defaultDisable(const ConcurrentAssertionStatement& statement) const {
    if (statement.assertionKind == AssertionKind::Expect || disableScopes.empty())
      return nullptr;
    const Scope& scope = *disableScopes.back();
    return scope.getCompilation().getDefaultDisable(scope);
  }

  template<typename T>
    requires(std::derived_from<T, TimingControl> ||
             std::derived_from<T, Constraint> ||
             std::derived_from<T, AssertionExpr> ||
             std::derived_from<T, Pattern>)
  void handle(const T& node) {
    if (capture.declarationOnly) {
      bindInvalidChildNames(node);
      visitDefault(node);
      return;
    }
    const uint64_t id = capture.ensureSemantic(&node);
    attach(id);
    if (!markVisited(id))
      return;
    bindInvalidChildNames(node);
    auto& result = capture.output.semantic_nodes[static_cast<size_t>(id)];
    if constexpr (std::derived_from<T, TimingControl>) {
      result.kind = LLG_SLANG_SEMANTIC_TIMING_CONTROL;
      result.subkind = semanticTimingKind(node.kind);
      if constexpr (std::same_as<T, BlockEventListControl>) {
        if (node.events.size() > 64)
          throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                              "block event list has more than 64 events");
        uint64_t begins = 0;
        for (size_t index = 0; index < node.events.size(); ++index) {
          if (node.events[index].isBegin)
            begins |= uint64_t(1) << index;
        }
        result.auxiliary = begins;
      }
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
      // Slang binds a parenthesized sequence or property to its operand and
      // records the parenthesized syntax as the operand's syntax.
      if (node.syntax &&
          (node.syntax->kind == syntax::SyntaxKind::ParenthesizedSequenceExpr ||
           node.syntax->kind == syntax::SyntaxKind::ParenthesizedPropertyExpr))
        result.auxiliary |= LLG_SLANG_ASSERTION_PARENTHESIZED;
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
    else if constexpr (std::derived_from<T, Constraint>) {
      const uint32_t subkind = semanticConstraintKind(node.kind);
      result.kind = subkind ? LLG_SLANG_SEMANTIC_CONSTRAINT
                            : LLG_SLANG_SEMANTIC_UNSUPPORTED;
      result.subkind = subkind;
      if constexpr (std::same_as<T, ExpressionConstraint>) {
        if (node.isSoft)
          result.auxiliary |= LLG_SLANG_CONSTRAINT_SOFT;
      }
      if constexpr (std::same_as<T, ForeachConstraint>)
        result.auxiliary = node.loopDims.size();
    }
    else {
      result.kind = LLG_SLANG_SEMANTIC_UNSUPPORTED;
      if constexpr (std::derived_from<T, Pattern>) {
        result.subkind = semanticPatternKind(node.kind);
        if constexpr (std::same_as<T, VariablePattern>) {
          const uint64_t target = captureReferenceTarget(node.variable);
          result.target_id = target;
          result.type_id = capture.type(node.variable.getType());
          // Pattern variables are lexical automatic temporaries. Slang uses
          // TempVarSymbol for them, so they do not pass through the ordinary
          // VariableSymbol lifetime capture path.
          if (target < capture.output.semantic_nodes.size()) {
            auto& target_node = capture.output.semantic_nodes[static_cast<size_t>(target)];
            target_node.subkind = LLG_SLANG_VARIABLE_PATTERN_BINDING;
            target_node.auxiliary = LLG_SLANG_VARIABLE_LIFETIME_AUTOMATIC;
            target_node.flags |= LLG_SLANG_SEMANTIC_AUTOMATIC;
          }
        }
        if constexpr (std::same_as<T, TaggedPattern>) {
          // A tagged pattern has two independent identities: the selected
          // union field and the optional payload pattern.  Keep the field as
          // the pattern target and make the payload an ordered operand so the
          // owned importer can validate the tag before lowering its payload.
          const uint64_t member = captureReferenceTarget(node.member);
          capture.output.semantic_nodes[static_cast<size_t>(id)].target_id =
              member;
        }
      }
    }
    result.detail = storeString(capture.output, toString(node.kind));
    if constexpr (requires { node.sourceRange; })
      result.range = capture.span(node.sourceRange);
    parents.push_back(id);
    // Loop variables are declarations of the foreach item; capturing them
    // can grow the node vector, so `result` is not used after this point.
    if constexpr (std::same_as<T, ForeachConstraint>) {
      for (const auto& dimension : node.loopDims) {
        if (dimension.loopVar)
          handle(*dimension.loopVar);
      }
    }
    visitDefault(node);
    parents.pop_back();
    if constexpr (std::derived_from<T, Constraint>)
      addConstraintRoles(id, node);
    // Block event targets are blocks or subroutines (19.3): export the
    // declarations themselves.
    if constexpr (std::same_as<T, BlockEventListControl>) {
      for (size_t index = 0; index < node.events.size(); ++index) {
        const Expression* target = node.events[index].target;
        const Symbol* symbol = target ? target->getSymbolReference() : nullptr;
        if (!symbol)
          continue;
        const uint64_t targetId = captureReferenceTarget(*symbol);
        capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE, targetId,
                             static_cast<uint32_t>(index));
      }
    }
    if constexpr (std::same_as<T, StructurePattern>) {
      // A structure pattern's field identities and nested patterns are
      // resolved by Slang. Replace generic child edges with paired roles so
      // the owned database can preserve named, reordered, omitted, and
      // recursive members without recovering source offsets.
      capture.removeChildEdges(id);
      uint32_t index = 0;
      for (const auto& fieldPattern : node.patterns) {
        const FieldSymbol* field = fieldPattern.field;
        const uint64_t fieldId = capture.ensureSemantic(field);
        auto& fieldNode =
            capture.output.semantic_nodes[static_cast<size_t>(fieldId)];
        if (fieldNode.name.len == 0) {
          fieldNode.kind = LLG_SLANG_SEMANTIC_VARIABLE;
          fieldNode.name = storeString(capture.output, field->name);
          fieldNode.detail = storeString(capture.output, "PatternField");
          fieldNode.range = capture.span(field->location, field->name.size());
        }
        fieldNode.type_id = capture.type(field->getType());
        capture.semanticRole(id, fieldPattern.pattern, LLG_SLANG_EDGE_OPERAND,
                             index);
        capture.semanticEdge(id, LLG_SLANG_EDGE_DECLARATION, fieldId, index);
        index++;
      }
    }
    if constexpr (std::same_as<T, TaggedPattern>) {
      // The generic visitor attaches the optional payload as a child. Replace
      // that structural edge after traversal with the owned ordered operand;
      // the union member remains the resolved target identity above.
      capture.removeChildEdges(id);
      if (node.valuePattern)
        capture.semanticRole(id, node.valuePattern, LLG_SLANG_EDGE_OPERAND);
    }
    addAuxiliaryRoles(node, id);
  }

private:
  // A typedef named as a cast target (`t'(x)`) or as a type operand (`$bits(t)`,
  // `type(t)`) is not a value reference, so no NamedValue expression carries it.
  // Slang's resolved type is the exact alias symbol; the name token is bound to
  // it only when the spelling matches that alias.
  void captureTypeNameReference(const Expression& expression) {
    if (!expression.syntax || !expression.type)
      return;
    const Type* named = expression.type;
    const syntax::SyntaxNode* spelling = nullptr;
    if (expression.syntax->kind == syntax::SyntaxKind::CastExpression) {
      spelling = expression.syntax->template as<syntax::CastExpressionSyntax>().left;
    }
    else if (expression.kind == ExpressionKind::DataType) {
      spelling = expression.syntax;
    }
    else if (expression.kind == ExpressionKind::TypeReference &&
             expression.syntax->kind == syntax::SyntaxKind::TypeReference) {
      named = &expression.template as<TypeReferenceExpression>().targetType;
      spelling = expression.syntax->template as<syntax::TypeReferenceSyntax>().expr;
    }
    if (!spelling || !named->isAlias())
      return;
    const auto token = spelling->getLastToken();
    if (token.valueText() == named->name)
      capture.lexicalBinding(token, captureReferenceTarget(*named),
                             LLG_SLANG_LEXICAL_ROLE_REFERENCE);
  }

  // The node of a symbol an expression references (getSymbolReference),
  // described from the symbol when nothing captured it yet: a struct member
  // is no scope member, so no traversal visits it.
  uint64_t describeReferenceTarget(const Symbol& target) {
    const uint64_t targetId = capture.ensureSemantic(&target);
    auto& targetNode = capture.output.semantic_nodes[static_cast<size_t>(targetId)];
    if (targetNode.name.len == 0) {
      targetNode.kind = semanticSymbolKind(target.kind);
      if (target.kind == SymbolKind::LocalAssertionVar)
        targetNode.subkind = LLG_SLANG_VARIABLE_ASSERTION_LOCAL;
      targetNode.name = storeString(capture.output, target.name);
      targetNode.detail = storeString(capture.output, toString(target.kind));
      targetNode.range = target.location.valid()
          ? capture.span(target.location, target.name.size())
          : LlgSlangSourceRange{LLG_SLANG_INVALID_ID, 0, 0};
    }
    if (targetNode.type_id == LLG_SLANG_INVALID_ID && ValueSymbol::isKind(target.kind))
      targetNode.type_id = capture.type(target.as<ValueSymbol>().getType());
    return targetId;
  }

  // Target node of a lexical binding. Declaration-only capture names the
  // node here, as its other syntax bindings do. Full capture keys it by
  // identity only, like its reference edges: the traversal fills every node it
  // visits, and consumers recognize a never-visited target (a built-in class
  // method such as mailbox `get`) by its unnamed node, so filling it here
  // would change what they see.
  uint64_t lexicalTarget(const Symbol& symbol) {
    return capture.declarationOnly ? captureReferenceTarget(symbol)
                                   : capture.ensureSemantic(&symbol);
  }

  // The width of a size cast `w'(x)` (IEEE 1800-2009 6.24.1) is a constant
  // expression that Slang binds, evaluates and discards, so no expression
  // carries its names; they are looked up again in the context of the
  // symbol or block being visited. A type cast is bound by
  // captureTypeNameReference instead: its target is a data type or names an
  // alias, which a size cast's integral result type never is.
  void bindSizeCastNames(const ConversionExpression& expression) {
    if (!lookupOwner || expression.isImplicit() || !expression.type ||
        expression.type->isAlias() ||
        expression.syntax->kind != syntax::SyntaxKind::CastExpression)
      return;
    const auto& left = *expression.syntax->template as<syntax::CastExpressionSyntax>().left;
    if (syntax::DataTypeSyntax::isKind(left.kind))
      return;
    if (const auto context = lookupContext())
      captureNames(left, *context);
  }

  // The lookup context of the symbol or block being visited (`lookupOwner`)
  // for names Slang binds and then discards: size cast widths and the value
  // assignments of a shared class specialization.
  std::optional<ASTContext> lookupContext() const {
    if (!lookupOwner)
      return std::nullopt;
    if (const Scope* scope = lookupOwner->as_if<Scope>())
      return ASTContext(*scope, LookupLocation::max);
    if (const Scope* scope = lookupOwner->getParentScope())
      return ASTContext(*scope, LookupLocation::after(*lookupOwner));
    return std::nullopt;
  }

  // Declaration-only capture keeps no expression nodes, so the token of each
  // resolved value reference is bound here instead; full capture derives the
  // same bindings from its reference nodes. Also used inside invalid subtrees,
  // which neither mode lowers.
  template<std::derived_from<Expression> T>
  void bindValueName(const T& expression) {
    if constexpr (std::same_as<T, NamedValueExpression>) {
      if (!expression.syntax)
        bindNameRange(expression.sourceRange, expression.symbol);
    }
    bindValueNameToken(expression, false);
    captureTypeNameReference(expression);
  }

  // Binds the value name token of a reference. With `macroArgumentsOnly`
  // (full capture) only a name written in a macro argument or copied from a
  // `define body is bound: its reference node spans the macro use site, so
  // the node cannot bind it.
  template<std::derived_from<Expression> T>
  void bindValueNameToken(const T& expression, bool macroArgumentsOnly) {
    if constexpr (std::same_as<T, NamedValueExpression> ||
                  std::same_as<T, HierarchicalValueExpression> ||
                  std::same_as<T, MemberAccessExpression>) {
      if (expression.syntax) {
        if (const Symbol* target = expression.getSymbolReference()) {
          // Slang drops redundant parentheses from the expression but keeps
          // them in its syntax (`(a)` in a macro body such as `((a) + (b))`).
          const syntax::SyntaxNode* name = expression.syntax;
          while (name->kind == syntax::SyntaxKind::ParenthesizedExpression)
            name = name->template as<syntax::ParenthesizedExpressionSyntax>().expression;
          auto token = name->getLastToken();
          // A value with member selects (`s.f`) carries the whole dotted
          // name as its syntax; the value is its leading step.
          if constexpr (std::same_as<T, NamedValueExpression>) {
            if (token.valueText() != target->name)
              token = valueNameToken(*name);
          }
          if (token.valueText() == target->name &&
              (!macroArgumentsOnly ||
               capture.macroArgumentSpan(token.location(), token.rawText()) ||
               capture.macroBodySpan(token.location(), token.rawText())))
            capture.lexicalBinding(token, lexicalTarget(*target),
                                   LLG_SLANG_LEXICAL_ROLE_REFERENCE);
        }
      }
    }
  }

  // Name tokens no reference node carries, bound in both capture modes:
  // scope qualifiers (`pk::pf`), hierarchical path steps (`u0.o`), symbol
  // operands such as a `disable` target, and subroutine names. A system task
  // or function (or built-in method) gets its own lexical kind with no target.
  template<std::derived_from<Expression> T>
  void bindNameTokens(const T& expression) {
    if constexpr (std::same_as<T, ArbitrarySymbolExpression>) {
      if (!expression.syntax)
        bindNameRange(expression.sourceRange, *expression.symbol);
    }
    if (!expression.syntax)
      return;
    if constexpr (std::same_as<T, NamedValueExpression>) {
      bindScopeQualifiers(*expression.syntax, expression.symbol);
    }
    else if constexpr (std::same_as<T, StructuredAssignmentPatternExpression>) {
      bindPatternMemberKeys(expression);
    }
    else if constexpr (std::same_as<T, MemberAccessExpression>) {
      // The inner selects of `s.a.b` and the value of `pk::v.f` may keep no
      // syntax of their own; their names are steps of the outer select's
      // name, so bind each member step right to left, and the qualifiers.
      std::vector<parsing::Token> names;
      collectNameTokens(*expression.syntax, names, false);
      size_t index = names.size();
      const Expression* value = &expression;
      while (value->kind == ExpressionKind::MemberAccess) {
        const auto& access = value->template as<MemberAccessExpression>();
        if (index == 0 || names[index - 1].valueText() != access.member.name ||
            access.member.name.empty())
          break;
        capture.lexicalBinding(names[--index], lexicalTarget(access.member),
                               LLG_SLANG_LEXICAL_ROLE_REFERENCE);
        value = &access.value();
      }
      const bool membersBound = value->kind != ExpressionKind::MemberAccess;
      while (value->kind == ExpressionKind::MemberAccess)
        value = &value->template as<MemberAccessExpression>().value();
      if (value->kind == ExpressionKind::NamedValue) {
        const Symbol& root = value->template as<NamedValueExpression>().symbol;
        if (membersBound && index > 0 && !root.name.empty() &&
            names[index - 1].valueText() == root.name)
          capture.lexicalBinding(names[index - 1], lexicalTarget(root),
                                 LLG_SLANG_LEXICAL_ROLE_REFERENCE);
        bindScopeQualifiers(*expression.syntax, root);
      }
      else if (value->kind == ExpressionKind::HierarchicalValue && membersBound &&
               index > 0) {
        // `u.p.f`: the hierarchical value `u.p` keeps no syntax here.
        const auto& hierarchical = value->template as<HierarchicalValueExpression>();
        if (names[index - 1].valueText() == hierarchical.symbol.name) {
          capture.lexicalBinding(names[index - 1], lexicalTarget(hierarchical.symbol),
                                 LLG_SLANG_LEXICAL_ROLE_REFERENCE);
          bindHierarchicalSteps(names, index, hierarchical.ref);
        }
      }
    }
    else if constexpr (std::same_as<T, HierarchicalValueExpression>) {
      bindHierarchicalPath(*expression.syntax, expression.ref);
    }
    else if constexpr (std::same_as<T, AssertionInstanceExpression>) {
      // A sequence, property or let instance (`p(a)`, `s`; IEEE 1800-2009
      // 16.8, 16.13, 11.13) names its declaration; the node spans the call.
      const syntax::SyntaxNode* name = expression.syntax;
      if (name->kind == syntax::SyntaxKind::InvocationExpression)
        name = name->template as<syntax::InvocationExpressionSyntax>().left;
      const auto token = name->getLastToken();
      if (!expression.symbol.name.empty() &&
          token.valueText() == expression.symbol.name) {
        capture.lexicalBinding(token, lexicalTarget(expression.symbol),
                               LLG_SLANG_LEXICAL_ROLE_REFERENCE);
        bindScopeQualifiers(*name, expression.symbol);
      }
    }
    else if constexpr (std::same_as<T, ConversionExpression>) {
      bindSizeCastNames(expression);
    }
    else if constexpr (std::same_as<T, ArbitrarySymbolExpression>) {
      const Symbol& target = *expression.symbol;
      const auto token = expression.syntax->getLastToken();
      if (!target.name.empty() && token.valueText() == target.name)
        capture.lexicalBinding(token, lexicalTarget(target),
                               LLG_SLANG_LEXICAL_ROLE_REFERENCE);
      if (expression.hierRef.target)
        bindHierarchicalPath(*expression.syntax, expression.hierRef);
      else
        bindScopeQualifiers(*expression.syntax, target);
    }
    else if constexpr (std::same_as<T, CallExpression>) {
      // `q.sum() with (item)` wraps the invocation `q.sum()` in the method
      // node, so unwrap both forms until the name itself is reached.
      const syntax::SyntaxNode* name = expression.syntax;
      while (true) {
        if (name->kind == syntax::SyntaxKind::InvocationExpression)
          name = name->template as<syntax::InvocationExpressionSyntax>().left;
        else if (name->kind == syntax::SyntaxKind::ArrayOrRandomizeMethodExpression)
          name = name->template as<syntax::ArrayOrRandomizeMethodExpressionSyntax>().method;
        else
          break;
      }
      const auto token = name->getLastToken();
      if (token.valueText() != expression.getSubroutineName())
        return;
      if (expression.isSystemCall()) {
        capture.lexicalBinding(token, LLG_SLANG_INVALID_ID,
                               LLG_SLANG_LEXICAL_ROLE_NONE,
                               LLG_SLANG_LEXICAL_SYSTEM_SUBROUTINE);
        // `std::randomize` (IEEE 1800-2009 18.12) names the built-in std
        // package, which has no source to navigate to: give the qualifier the
        // package kind without a target.
        if (name->kind == syntax::SyntaxKind::ScopedName) {
          const auto& scoped = name->template as<syntax::ScopedNameSyntax>();
          const auto qualifier = scoped.left->getLastToken();
          if (scoped.separator.kind == parsing::TokenKind::DoubleColon &&
              scoped.left->kind == syntax::SyntaxKind::IdentifierName &&
              qualifier.valueText() == "std")
            capture.lexicalBinding(qualifier, LLG_SLANG_INVALID_ID,
                                   LLG_SLANG_LEXICAL_ROLE_NONE,
                                   LLG_SLANG_LEXICAL_PACKAGE);
        }
        return;
      }
      const SubroutineSymbol* target = std::get<0>(expression.subroutine);
      if (!target)
        return;
      // A method Slang builds without source is built in: the methods of the
      // std package classes (`process`, `semaphore`, `mailbox`; IEEE
      // 1800-2009 9.7, 15.1 "These built-in classes reside in the built-in
      // std package") and the implicit class methods (`srandom`,
      // `get_randstate`, `set_randstate`, 18.13.3 to 18.13.5). It is sent
      // like a system method; a user override has syntax and stays a
      // reference.
      if (!target->getSyntax()) {
        capture.lexicalBinding(token, LLG_SLANG_INVALID_ID,
                               LLG_SLANG_LEXICAL_ROLE_NONE,
                               LLG_SLANG_LEXICAL_SYSTEM_SUBROUTINE);
      }
      else {
        capture.lexicalBinding(token, lexicalTarget(*target),
                               LLG_SLANG_LEXICAL_ROLE_REFERENCE);
      }
      bindScopeQualifiers(*name, *target);
    }
  }

  // A member key of a structure assignment pattern (`'{a: 1}`, IEEE
  // 1800-2009 10.9.2) names a member of the pattern's type; Slang keeps the
  // member but not the key's syntax, so match the written keys by name.
  void bindPatternMemberKeys(const StructuredAssignmentPatternExpression& expression) {
    if (expression.memberSetters.empty())
      return;
    const syntax::SyntaxNode* pattern = expression.syntax;
    if (pattern->kind == syntax::SyntaxKind::AssignmentPatternExpression)
      pattern = pattern->as<syntax::AssignmentPatternExpressionSyntax>().pattern;
    if (pattern->kind != syntax::SyntaxKind::StructuredAssignmentPattern)
      return;
    for (const syntax::AssignmentPatternItemSyntax* item :
         pattern->as<syntax::StructuredAssignmentPatternSyntax>().items) {
      if (item->key->kind != syntax::SyntaxKind::IdentifierName)
        continue;
      const parsing::Token key =
          item->key->as<syntax::IdentifierNameSyntax>().identifier;
      for (const auto& setter : expression.memberSetters) {
        if (setter.member->name == key.valueText()) {
          capture.lexicalBinding(key, lexicalTarget(*setter.member),
                                 LLG_SLANG_LEXICAL_ROLE_REFERENCE);
          break;
        }
      }
    }
  }

  // The interface name of an interface port header (`ifc bus`, `ifc.mp bus`,
  // IEEE 1800-2009 25.3) names the definition the port resolved to, like an
  // instantiation's type name.
  void bindInterfacePortType(const InterfacePortSymbol& port) {
    const syntax::SyntaxNode* syntaxNode = port.getSyntax();
    if (syntaxNode && syntaxNode->kind != syntax::SyntaxKind::ImplicitAnsiPort)
      syntaxNode = syntaxNode->parent;
    if (!port.interfaceDef || !syntaxNode ||
        syntaxNode->kind != syntax::SyntaxKind::ImplicitAnsiPort)
      return;
    const auto& header =
        *syntaxNode->as<syntax::ImplicitAnsiPortSyntax>().header;
    parsing::Token token;
    if (header.kind == syntax::SyntaxKind::InterfacePortHeader) {
      token = header.as<syntax::InterfacePortHeaderSyntax>().nameOrKeyword;
    }
    else if (header.kind == syntax::SyntaxKind::VariablePortHeader) {
      // `ifc bus` parses as a named data type; elaboration finds the
      // interface definition.
      const auto& type = *header.as<syntax::VariablePortHeaderSyntax>().dataType;
      if (type.kind != syntax::SyntaxKind::NamedType)
        return;
      const auto& name = *type.as<syntax::NamedTypeSyntax>().name;
      if (name.kind == syntax::SyntaxKind::IdentifierName)
        token = name.as<syntax::IdentifierNameSyntax>().identifier;
    }
    if (token.kind == parsing::TokenKind::Identifier &&
        token.valueText() == port.interfaceDef->name)
      capture.lexicalBinding(token, capture.ensureSemantic(port.interfaceDef),
                             LLG_SLANG_LEXICAL_ROLE_REFERENCE);
  }

  // A `disable` target (IEEE 1800-2009 9.6.2) names a block or task, possibly
  // hierarchically; its operand expression keeps no syntax, the statement does.
  template<std::derived_from<Statement> T>
  void bindStatementNames(const T& statement) {
    if constexpr (std::same_as<T, DisableStatement>) {
      if (!statement.syntax ||
          statement.syntax->kind != syntax::SyntaxKind::DisableStatement ||
          statement.target.kind != ExpressionKind::ArbitrarySymbol)
        return;
      const auto& name = *statement.syntax->template as<syntax::DisableStatementSyntax>().name;
      const auto& target = statement.target.template as<ArbitrarySymbolExpression>();
      const auto token = name.getLastToken();
      if (token.valueText() == target.symbol->name)
        capture.lexicalBinding(token, lexicalTarget(*target.symbol),
                               LLG_SLANG_LEXICAL_ROLE_REFERENCE);
      if (target.hierRef.target)
        bindHierarchicalPath(name, target.hierRef);
    }
  }

  // An expression Slang builds inside a larger name (the value of `s.f`, a
  // symbol operand) keeps no syntax. Its range is its name token exactly when
  // the name is one identifier spelled as the symbol's name.
  void bindNameRange(SourceRange range, const Symbol& target) {
    if (target.name.empty() || !range.start().valid() ||
        !capture.sourceManager.isFileLoc(range.start()) ||
        range.start().buffer() != range.end().buffer() ||
        range.end().offset() - range.start().offset() != target.name.size())
      return;
    const std::string_view text =
        capture.sourceManager.getSourceText(range.start().buffer());
    if (range.end().offset() > text.size() ||
        text.substr(range.start().offset(), target.name.size()) != target.name)
      return;
    capture.lexicalBinding(range, lexicalTarget(target),
                           LLG_SLANG_LEXICAL_ROLE_REFERENCE);
  }

  // The value step of a name: Slang nests scoped names to the left, so member
  // selects (`.`) hang off the left operand and the value follows the last
  // `::` qualifier.
  static parsing::Token valueNameToken(const syntax::SyntaxNode& name) {
    switch (name.kind) {
      case syntax::SyntaxKind::ScopedName: {
        const auto& scoped = name.as<syntax::ScopedNameSyntax>();
        return valueNameToken(scoped.separator.kind == parsing::TokenKind::Dot
                                  ? *scoped.left
                                  : *scoped.right);
      }
      case syntax::SyntaxKind::IdentifierName:
        return name.as<syntax::IdentifierNameSyntax>().identifier;
      case syntax::SyntaxKind::IdentifierSelectName:
        return name.as<syntax::IdentifierSelectNameSyntax>().identifier;
      default: return {};
    }
  }

  // The identifier token of each step of a name (`a::b`, `a.b[0].c`), in
  // source order. Keyword steps such as `$unit`, `$root`, `this` or `super`
  // have no identifier and are skipped, as are selects and parameter lists.
  static void collectNameTokens(const syntax::SyntaxNode& name,
                                std::vector<parsing::Token>& tokens,
                                bool qualifiersOnly) {
    switch (name.kind) {
      case syntax::SyntaxKind::ScopedName: {
        const auto& scoped = name.as<syntax::ScopedNameSyntax>();
        if (qualifiersOnly &&
            scoped.separator.kind != parsing::TokenKind::DoubleColon)
          return;
        collectNameTokens(*scoped.left, tokens, qualifiersOnly);
        collectNameTokens(*scoped.right, tokens, qualifiersOnly);
        break;
      }
      case syntax::SyntaxKind::IdentifierName:
        tokens.push_back(name.as<syntax::IdentifierNameSyntax>().identifier);
        break;
      case syntax::SyntaxKind::IdentifierSelectName:
        tokens.push_back(name.as<syntax::IdentifierSelectNameSyntax>().identifier);
        break;
      case syntax::SyntaxKind::ClassName:
        tokens.push_back(name.as<syntax::ClassNameSyntax>().identifier);
        break;
      default: break;
    }
  }

  // An import or export item `pkg::name` / `pkg::*` (IEEE 1800-2009 26.3,
  // 26.6) names a package and, unless it is `*`, one of its members. Neither
  // name declares anything in the importing scope; Slang's import symbol sits
  // at the item name, so without these bindings the member name would read as
  // a declaration of its own. An unresolved package or member stays unbound.
  template<typename T>
  void bindImportItem(const T& symbol) {
    const syntax::SyntaxNode* syntaxNode = symbol.getSyntax();
    if (!syntaxNode || syntaxNode->kind != syntax::SyntaxKind::PackageImportItem)
      return;
    const auto& item = syntaxNode->as<syntax::PackageImportItemSyntax>();
    const PackageSymbol* package = nullptr;
    if constexpr (std::same_as<T, ExplicitImportSymbol>)
      package = symbol.package();
    else
      package = symbol.getPackage();
    // A name a macro builds is not spelled at its token; leave it unbound.
    if (!package || item.package.valueText() != package->name)
      return;
    capture.lexicalBinding(item.package, lexicalTarget(*package),
                           LLG_SLANG_LEXICAL_ROLE_REFERENCE);
    if constexpr (std::same_as<T, ExplicitImportSymbol>) {
      const Symbol* imported = symbol.importedSymbol();
      // An enum value is visible in its package through a transparent
      // member; references elsewhere bind the value itself.
      if (imported && imported->kind == SymbolKind::TransparentMember)
        imported = &imported->template as<TransparentMemberSymbol>().wrapped;
      if (imported && item.item.valueText() == imported->name)
        capture.lexicalBinding(item.item, lexicalTarget(*imported),
                               LLG_SLANG_LEXICAL_ROLE_REFERENCE);
    }
  }

  // `a::b::name` (IEEE 1800-2009 23.7, 8.23): each qualifier names a scope
  // that encloses the resolved target, a package or class (an enum value is
  // reached through its enum type's enclosing scope). Bind each qualifier,
  // right to left, to the nearest enclosing scope of that name; stop at the
  // first qualifier that names none.
  void bindScopeQualifiers(const syntax::SyntaxNode& syntaxNode,
                           const Symbol& target) {
    // A value with member selects (`pk::v.f`) carries the whole dotted name;
    // the qualifiers belong to its leading `pk::v`.
    const syntax::SyntaxNode* name = &syntaxNode;
    while (name->kind == syntax::SyntaxKind::ScopedName &&
           name->as<syntax::ScopedNameSyntax>().separator.kind ==
               parsing::TokenKind::Dot)
      name = name->as<syntax::ScopedNameSyntax>().left;
    if (name->kind != syntax::SyntaxKind::ScopedName)
      return;
    const auto& scoped = name->as<syntax::ScopedNameSyntax>();
    if (scoped.separator.kind != parsing::TokenKind::DoubleColon)
      return;
    std::vector<parsing::Token> qualifiers;
    collectNameTokens(*scoped.left, qualifiers, true);
    const Scope* scope = target.getParentScope();
    for (auto token = qualifiers.rbegin(); token != qualifiers.rend(); ++token) {
      while (scope && scope->asSymbol().name != token->valueText())
        scope = scope->asSymbol().getParentScope();
      if (!scope)
        return;
      const Symbol& qualifier = scope->asSymbol();
      if (qualifier.kind == SymbolKind::CompilationUnit ||
          qualifier.kind == SymbolKind::Root)
        return;
      capture.lexicalBinding(*token, lexicalTarget(qualifier),
                             LLG_SLANG_LEXICAL_ROLE_REFERENCE);
      if (qualifier.kind == SymbolKind::ClassType)
        bindSpecializationValues(qualifier.as<ClassType>(), *scoped.left);
      scope = qualifier.getParentScope();
    }
  }

  // The value parameter assignments of a class specialization `C#(e)::name`
  // (IEEE 1800-2009 8.25.1) are bound by Slang in the referencing scope, and
  // the overridden parameter's initializer keeps that binding; its names are
  // bound from it when this use site wrote them. Slang shares one
  // specialization between assignments with equal values, so the text of
  // any other site has no initializer: its names are looked up in the
  // context of the symbol or block being visited (`lookupContext`), as size
  // cast widths are.
  void bindSpecializationValues(const ClassType& type,
                                const syntax::SyntaxNode& qualifier) {
    if (!type.genericClass)
      return;
    const SourceRange written = qualifier.sourceRange();
    const auto within = [&](SourceLocation location) {
      return location.valid() && written.start().valid() && written.end().valid() &&
             location.buffer() == written.start().buffer() &&
             location.buffer() == written.end().buffer() &&
             location.offset() >= written.start().offset() &&
             location.offset() < written.end().offset();
    };
    bool bound = false;
    for (const ParameterSymbol& parameter : type.membersOfType<ParameterSymbol>()) {
      if (!parameter.isOverridden())
        continue;
      const Expression* initializer = parameter.getInitializer();
      if (initializer && within(initializer->sourceRange.start())) {
        bindInvalidSubtree(initializer);
        bound = true;
      }
    }
    if (bound)
      return;
    const auto context = lookupContext();
    if (!context)
      return;
    auto visitor = syntax::makeSyntaxVisitor(
        [&](auto&, const syntax::ParameterValueAssignmentSyntax& assignment) {
          captureNames(assignment, *context);
        });
    qualifier.visit(visitor);
  }

  // A hierarchical name (IEEE 1800-2009 23.6) resolves through instances,
  // generate blocks and interface ports; Slang records each named step of the
  // resolved path. Bind every name token before the last, right to left, to
  // the step of that name (index steps such as `gb[0]` have no token). The
  // last token is the reference itself, bound like any other.
  void bindHierarchicalPath(const syntax::SyntaxNode& syntaxNode,
                            const HierarchicalReference& ref) {
    if (syntaxNode.kind != syntax::SyntaxKind::ScopedName || ref.path.empty())
      return;
    std::vector<parsing::Token> names;
    collectNameTokens(syntaxNode, names, false);
    bindHierarchicalSteps(names, names.size(), ref);
  }

  // Binds `names[0, count - 1)`, the steps before the reference's own name
  // `names[count - 1]`, as bindHierarchicalPath does.
  void bindHierarchicalSteps(const std::vector<parsing::Token>& names,
                             size_t count, const HierarchicalReference& ref) {
    if (count < 2 || ref.path.empty())
      return;
    size_t step = ref.path.size();
    if (ref.path[step - 1].symbol == ref.target)
      step--;
    for (size_t index = count - 1; index-- > 0;) {
      const std::string_view text = names[index].valueText();
      while (step > 0) {
        const auto& element = ref.path[--step];
        const auto* selector = std::get_if<std::string_view>(&element.selector);
        if (selector && *selector == text && !text.empty()) {
          capture.lexicalBinding(names[index],
                                 lexicalTarget(*element.symbol),
                                 LLG_SLANG_LEXICAL_ROLE_REFERENCE);
          break;
        }
      }
    }
  }

  // Slang wraps a statement or expression that failed to bind, together with
  // the statement list or block that holds it, in an Invalid node whose child
  // the AST visitor never enters, so one undeclared name would leave every
  // name of its block unbound. Neither capture mode lowers that subtree, but
  // the names in it that did resolve are still bound lexically; only the
  // unresolved name stays a plain identifier. No semantic node is added for
  // the subtree itself.
  class InvalidSubtreeNames final
      : public ASTVisitor<InvalidSubtreeNames,
                          VisitFlags::Statements | VisitFlags::Expressions |
                              VisitFlags::Bad> {
  public:
    explicit InvalidSubtreeNames(SemanticCapture& owner) : owner(owner) {}

    template<std::derived_from<Expression> T>
    void handle(const T& expression) {
      // Full capture describes a reference target where it captures the
      // reference; no node is captured here, so describe it as that would.
      if (!owner.capture.declarationOnly) {
        if (const Symbol* target = expression.getSymbolReference())
          owner.describeReferenceTarget(*target);
      }
      owner.bindValueName(expression);
      owner.bindNameTokens(expression);
      if constexpr (std::same_as<T, InvalidExpression>) {
        if (expression.child)
          expression.child->visit(*this);
      }
      else {
        visitDefault(expression);
      }
    }

    template<std::derived_from<Statement> T>
    void handle(const T& statement) {
      owner.bindStatementNames(statement);
      if constexpr (std::same_as<T, InvalidStatement>) {
        if (statement.child)
          statement.child->visit(*this);
      }
      else {
        visitDefault(statement);
      }
    }

    // Event controls and assertion expressions in the subtree may wrap their
    // own invalid child (bindInvalidChildNames).
    void handle(const InvalidTimingControl& timing) {
      if (timing.child)
        timing.child->visit(*this);
    }

    void handle(const InvalidAssertionExpr& expression) {
      if (expression.child)
        expression.child->visit(*this);
    }

    // Declarations are captured from their scopes, never from here.
    template<std::derived_from<Symbol> T>
    void handle(const T&) {}

  private:
    SemanticCapture& owner;
  };

  template<typename T>
  void bindInvalidSubtree(const T* child) {
    if (!child)
      return;
    InvalidSubtreeNames names(*this);
    child->visit(names);
  }

  // An event control or assertion expression in which one name failed to
  // bind (`@(a or ghost)`, `@(posedge clk iff ghost)`) is wrapped, like an
  // invalid statement, in an Invalid node whose child no visitor enters, so
  // the names in it that did resolve are bound here (lexically only).
  template<typename T>
  void bindInvalidChildNames(const T& node) {
    if constexpr (std::same_as<T, InvalidTimingControl> ||
                  std::same_as<T, InvalidAssertionExpr>)
      bindInvalidSubtree(node.child);
  }

  void captureNames(const syntax::SyntaxNode& syntaxNode, const ASTContext& context) {
    auto visitor = syntax::makeSyntaxVisitor(
        [&](auto&, const syntax::NameSyntax& name) {
          LookupResult result;
          Lookup::name(name, context, LookupFlags::None, result);
          if (!result.found || result.hasError() || !result.selectors.empty())
            return;
          const auto token = name.getLastToken();
          if (token.valueText() == result.found->name) {
            capture.lexicalBinding(token, captureReferenceTarget(*result.found),
                                   LLG_SLANG_LEXICAL_ROLE_REFERENCE);
            bindScopeQualifiers(name, *result.found);
          }
        });
    syntaxNode.visit(visitor);
  }

  // The names written in a sequence, property or let declaration (IEEE
  // 1800-2009 16.8, 16.13, 11.13). Slang binds the body only per instance,
  // replacing each formal reference by the instance's actual argument
  // (stamped with the formal reference's syntax), so the instance graph
  // would bind a formal to an actual of one instance and leave a declaration
  // that no assertion instantiates unbound. Every name is looked up instead
  // in the declaration's own scope, where Slang binds the body
  // (AssertionInstanceExpression::fromLookup: `bodyContext(*symbolScope,
  // LookupLocation::max)`), so a formal binds to the formal and every other
  // name to what each instance sees. Lexical bindings only; each declaration
  // is bound once, whichever instance body holds it.
  void bindAssertionDeclarationNames(const Symbol& symbol, const Scope& scope) {
    const syntax::SyntaxNode* declaration = symbol.getSyntax();
    if (!declaration || !assertionDeclarations.insert(declaration).second)
      return;
    const ASTContext context(scope, LookupLocation::max);
    LookupNames names(*this, context);
    declaration->visit(names);
  }

  // Binds the name tokens of one written name (`x`, `w[i]`, `pk::v`,
  // `dut.bridge_mode`, `s.f`) looked up in `context`: the token of the
  // symbol found, its scope qualifiers and its hierarchical path steps.
  // Member names after the found symbol stay to the expression bindings of
  // the instances.
  void bindLookupName(const syntax::NameSyntax& name, const ASTContext& context) {
    LookupResult result;
    Lookup::name(name, context, LookupFlags::None, result);
    if (!result.found || result.hasError())
      return;
    const Symbol* found = result.found;
    if (found->kind == SymbolKind::TransparentMember)
      found = &found->as<TransparentMemberSymbol>().wrapped;
    if (found->name.empty())
      return;
    std::vector<parsing::Token> tokens;
    collectNameTokens(name, tokens, false);
    // The found symbol's token is the last one inside the part of the name
    // the lookup resolved; selects (`.f`, `[i]`) follow it.
    size_t count = tokens.size();
    const SourceLocation resolvedEnd = result.nameRange.end();
    if (!result.selectors.empty() && resolvedEnd.valid()) {
      while (count > 0 &&
             tokens[count - 1].location().buffer() == resolvedEnd.buffer() &&
             tokens[count - 1].location().offset() >= resolvedEnd.offset())
        count--;
    }
    if (count == 0 || tokens[count - 1].valueText() != found->name)
      return;
    capture.lexicalBinding(tokens[count - 1], lexicalTarget(*found),
                           LLG_SLANG_LEXICAL_ROLE_REFERENCE);
    if (!result.path.empty()) {
      HierarchicalReference reference;
      reference.target = found;
      reference.path = std::span<const HierarchicalReference::Element>(
          result.path.data(), result.path.size());
      bindHierarchicalSteps(tokens, count, reference);
    }
    else {
      bindScopeQualifiers(name, *found);
    }
  }

  // Binds every name written in a syntax subtree by looking it up in one
  // context (bindLookupName): assertion declarations and connection actuals
  // that Slang never binds.
  class LookupNames final : public syntax::SyntaxVisitor<LookupNames> {
  public:
    LookupNames(SemanticCapture& owner, const ASTContext& context)
        : owner(owner), context(context) {}

    void handle(const syntax::IdentifierNameSyntax& name) {
      owner.bindLookupName(name, context);
    }

    void handle(const syntax::IdentifierSelectNameSyntax& name) {
      owner.bindLookupName(name, context);
      visitSelects(name);
    }

    void handle(const syntax::ScopedNameSyntax& name) {
      owner.bindLookupName(name, context);
      visitSelects(name);
    }

  private:
    // The index expressions of a name's selects are names of their own;
    // the steps of the name itself are bound by bindLookupName.
    void visitSelects(const syntax::SyntaxNode& name) {
      switch (name.kind) {
        case syntax::SyntaxKind::ScopedName: {
          const auto& scoped = name.as<syntax::ScopedNameSyntax>();
          visitSelects(*scoped.left);
          visitSelects(*scoped.right);
          break;
        }
        case syntax::SyntaxKind::IdentifierSelectName:
          for (const syntax::ElementSelectSyntax* select :
               name.as<syntax::IdentifierSelectNameSyntax>().selectors)
            select->visit(*this);
          break;
        case syntax::SyntaxKind::ClassName:
          name.as<syntax::ClassNameSyntax>().parameters->visit(*this);
          break;
        default: break;
      }
    }

    SemanticCapture& owner;
    const ASTContext& context;
  };

  // Sequence, property and let declarations already bound
  // (bindAssertionDeclarationNames).
  std::unordered_set<const syntax::SyntaxNode*> assertionDeclarations;

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

  // Specify path capture (IEEE 1364-2001 14.2, IEEE 1800-2009 30.4).
  // Slang's visitor does not descend into a timing path's expressions; each
  // one is captured here under the path node (the caller has pushed it).
  void specifyOperand(uint64_t owner, const Expression& expression, uint32_t role,
                      uint32_t index = 0) {
    expression.visit(*this);
    capture.semanticRole(owner, &expression, role, index);
  }

  void captureTimingPath(uint64_t id, const TimingPathSymbol& path) {
    uint32_t index = 0;
    for (const Expression* input : path.getInputs())
      specifyOperand(id, *input, LLG_SLANG_EDGE_OPERAND, index++);
    index = 0;
    for (const Expression* output : path.getOutputs())
      specifyOperand(id, *output, LLG_SLANG_EDGE_LHS, index++);
    index = 0;
    for (const Expression* delay : path.getDelays())
      specifyOperand(id, *delay, LLG_SLANG_EDGE_DELAY, index++);
    if (const Expression* condition = path.getConditionExpr())
      specifyOperand(id, *condition, LLG_SLANG_EDGE_CONDITION);
    if (const Expression* source = path.getEdgeSourceExpr())
      specifyOperand(id, *source, LLG_SLANG_EDGE_RHS);
  }

  // Timing check arguments (IEEE 1800-2009 31): each event terminal and its
  // condition, each limit, the notifier and the $setuphold/$recrem
  // timestamp/timecheck conditions and delayed signals, under the check
  // node. Slang's visitor does not descend into them either.
  void captureTimingCheck(uint64_t id, const SystemTimingCheckSymbol& check) {
    const auto formals = timingCheckFormals(check.timingCheckKind);
    const auto args = check.getArguments();
    // The frontend binds the notifier and the delayed signals as output
    // arguments: assignments whose left side is the variable or net.
    auto target = [](const Expression& expression) -> const Expression& {
      if (expression.kind == ExpressionKind::Assignment)
        return expression.as<AssignmentExpression>().left();
      return expression;
    };
    uint32_t event = 0, limit = 0, condition = 2, delayed = 1;
    for (size_t index = 0; index < args.size() && index < formals.size(); index++) {
      const SystemTimingCheckSymbol::Arg& arg = args[index];
      switch (formals[index]) {
        case TimingArg::Event:
          if (arg.expr)
            specifyOperand(id, *arg.expr, LLG_SLANG_EDGE_OPERAND, event);
          if (arg.condition)
            specifyOperand(id, *arg.condition, LLG_SLANG_EDGE_CONDITION, event);
          event++;
          break;
        case TimingArg::Limit:
          if (arg.expr)
            specifyOperand(id, *arg.expr, LLG_SLANG_EDGE_DELAY, limit);
          limit++;
          break;
        case TimingArg::Notifier:
          if (arg.expr)
            specifyOperand(id, target(*arg.expr), LLG_SLANG_EDGE_LHS, 0);
          break;
        case TimingArg::Condition:
          if (arg.expr)
            specifyOperand(id, *arg.expr, LLG_SLANG_EDGE_CONDITION, condition);
          condition++;
          break;
        case TimingArg::Delayed:
          if (arg.expr)
            specifyOperand(id, target(*arg.expr), LLG_SLANG_EDGE_LHS, delayed);
          delayed++;
          break;
      }
    }
  }

  // Randsequence capture (IEEE 1800-2009 18.17). Rules and production-list
  // elements have no symbol identity, so each becomes a synthetic
  // LLG_SLANG_SEMANTIC_PRODUCTION node owned by its production or rule. The
  // assignments that bind formals and copy results are synthesized as Slang
  // expressions in the compilation, so their capture is the ordinary
  // assignment capture; see LLG_SLANG_PRODUCTION_* in the header.

  static bool isProductionFormal(const FormalArgumentSymbol& formal) {
    const Scope* parent = formal.getParentScope();
    return parent && parent->asSymbol().kind == SymbolKind::RandSeqProduction;
  }

  // The automatic variable that holds a non-void production's `return`
  // value until the generating rule copies it to its implicit variable.
  // Created on first use: a rule may reference a production captured later.
  const VariableSymbol* productionResult(const RandSeqProductionSymbol& production) {
    const Type& type = production.getReturnType();
    if (type.isVoid() || type.isError())
      return nullptr;
    auto [it, inserted] = productionResults.try_emplace(&production, nullptr);
    if (inserted) {
      auto& compilation = production.getCompilation();
      auto* variable = compilation.emplace<VariableSymbol>(
          production.name, production.location, VariableLifetime::Automatic);
      variable->setType(type);
      it->second = variable;
    }
    return it->second;
  }

  // `variable = fresh`, where `fresh` is a never-written automatic variable
  // of the same type owned by `owner`: it resets `variable` to its type's
  // default, so every generation of a production starts with fresh automatic
  // values (18.17, 18.17.7) whatever the type.
  Expression& productionReset(Compilation& compilation, uint64_t owner,
                              const VariableSymbol& variable, SourceRange range) {
    auto* fresh = compilation.emplace<VariableSymbol>(variable.name, variable.location,
                                                      VariableLifetime::Automatic);
    fresh->setType(variable.getType());
    parents.push_back(owner);
    handle(*fresh);
    parents.pop_back();
    auto& destination = *compilation.emplace<NamedValueExpression>(variable, range);
    auto& source = *compilation.emplace<NamedValueExpression>(*fresh, range);
    return *compilation.emplace<AssignmentExpression>(
        std::nullopt, false, variable.getType(), destination, source, nullptr, range);
  }

  uint64_t productionNode(uint64_t owner, uint32_t subkind, uint64_t auxiliary,
                          SourceRange range, std::string_view detail) {
    const uint64_t id = capture.newSyntheticSemantic();
    auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
    node.parent_id = owner;
    node.kind = LLG_SLANG_SEMANTIC_PRODUCTION;
    node.subkind = subkind;
    node.auxiliary = auxiliary;
    node.range = capture.span(range);
    node.detail = storeString(capture.output, detail);
    return id;
  }

  // Visit `expression` under `owner` and give it `role`.
  void productionOperand(uint64_t owner, const Expression& expression,
                         uint32_t role, uint32_t index = 0) {
    parents.push_back(owner);
    expression.visit(*this);
    parents.pop_back();
    capture.semanticRole(owner, &expression, role, index);
  }

  void captureProduction(uint64_t id, const RandSeqProductionSymbol& production) {
    auto& compilation = production.getCompilation();
    const SourceRange range = production.getSyntax()
                                  ? production.getSyntax()->sourceRange()
                                  : SourceRange(production.location, production.location);
    uint64_t nonInput = 0;
    uint32_t index = 0;
    for (const FormalArgumentSymbol* formal : production.arguments) {
      parents.push_back(id);
      handle(*formal);
      parents.pop_back();
      capture.semanticRole(id, formal, LLG_SLANG_EDGE_ARGUMENT, index);
      if (formal->direction != ArgumentDirection::In)
        nonInput |= 1ull << std::min<uint32_t>(index, 63);
      auto& reference = *compilation.emplace<NamedValueExpression>(
          *formal, SourceRange(formal->location, formal->location));
      productionOperand(id, reference, LLG_SLANG_EDGE_OPERAND, index);
      index++;
    }
    capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary = nonInput;
    if (const VariableSymbol* result = productionResult(production)) {
      parents.push_back(id);
      handle(*result);
      parents.pop_back();
      capture.semanticRole(id, result, LLG_SLANG_EDGE_DECLARATION);
      auto& target = *compilation.emplace<NamedValueExpression>(*result, range);
      productionOperand(id, target, LLG_SLANG_EDGE_LHS);
      productionOperand(id, productionReset(compilation, id, *result, range),
                        LLG_SLANG_EDGE_DEFAULT_VALUE);
    }
    index = 0;
    for (const auto& rule : production.getRules()) {
      const auto* syntaxNode = rule.ruleBlock->getSyntax();
      if (!syntaxNode || syntaxNode->kind != syntax::SyntaxKind::RsRule)
        throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                            "randsequence rule has no rule syntax");
      const auto& ruleSyntax = syntaxNode->as<syntax::RsRuleSyntax>();
      const uint64_t ruleId = productionNode(
          id, LLG_SLANG_PRODUCTION_RULE,
          rule.isRandJoin ? LLG_SLANG_PRODUCTION_RULE_RAND_JOIN : 0,
          ruleSyntax.sourceRange(), "RandSeqRule");
      capture.semanticEdge(id, LLG_SLANG_EDGE_BRANCH, ruleId, index++);
      captureRule(ruleId, rule, ruleSyntax);
    }
  }

  void captureRule(uint64_t id, const RandSeqProductionSymbol::Rule& rule,
                   const syntax::RsRuleSyntax& ruleSyntax) {
    // Implicit result variables of the productions this rule generates.
    std::unordered_map<std::string_view, const VariableSymbol*> variables;
    uint32_t index = 0;
    for (const Symbol& member : rule.ruleBlock->members()) {
      if (member.kind != SymbolKind::Variable)
        continue;
      const auto& variable = member.as<VariableSymbol>();
      if (!variable.flags.has(VariableFlags::CompilerGenerated))
        continue;
      parents.push_back(id);
      handle(variable);
      parents.pop_back();
      capture.semanticRole(id, &variable, LLG_SLANG_EDGE_DECLARATION, index);
      productionOperand(id,
                        productionReset(rule.ruleBlock->getCompilation(), id, variable,
                                        ruleSyntax.sourceRange()),
                        LLG_SLANG_EDGE_INITIALIZER, index);
      index++;
      variables.emplace(variable.name, &variable);
    }
    if (rule.weightExpr)
      productionOperand(id, *rule.weightExpr, LLG_SLANG_EDGE_WEIGHT);
    if (rule.randJoinExpr)
      productionOperand(id, *rule.randJoinExpr, LLG_SLANG_EDGE_RAND_JOIN);
    // Occurrences number the elements of an implicit array from 1 in
    // syntactic order, counting if/else, repeat and case items (18.17.7).
    std::unordered_map<const RandSeqProductionSymbol*, int32_t> occurrences;
    RuleContext context{*rule.ruleBlock, variables, occurrences};
    if (rule.prods.size() != ruleSyntax.prods.size())
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "randsequence rule elements disagree with their syntax");
    for (size_t element = 0; element < rule.prods.size(); element++) {
      const uint64_t elementId =
          captureElement(id, *rule.prods[element], *ruleSyntax.prods[element], context);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, elementId,
                           static_cast<uint32_t>(element));
    }
    if (ruleSyntax.weightClause && ruleSyntax.weightClause->codeBlock) {
      const uint64_t codeId = captureCodeBlock(
          id, *rule.ruleBlock, *ruleSyntax.weightClause->codeBlock);
      capture.semanticEdge(id, LLG_SLANG_EDGE_BODY, codeId);
    }
  }

  struct RuleContext {
    const StatementBlockSymbol& block;
    const std::unordered_map<std::string_view, const VariableSymbol*>& variables;
    std::unordered_map<const RandSeqProductionSymbol*, int32_t>& occurrences;
  };

  uint64_t captureElement(uint64_t rule, const RandSeqProductionSymbol::ProdBase& prod,
                          const syntax::RsProdSyntax& prodSyntax, RuleContext& context) {
    using ProdKind = RandSeqProductionSymbol::ProdKind;
    switch (prod.kind) {
      case ProdKind::Item:
        return captureItem(rule, prod.as<RandSeqProductionSymbol::ProdItem>(),
                           prodSyntax.as<syntax::RsProdItemSyntax>(), &context);
      case ProdKind::CodeBlock:
        return captureCodeBlock(rule, context.block, prodSyntax);
      case ProdKind::IfElse: {
        const auto& ifElse = prod.as<RandSeqProductionSymbol::IfElseProd>();
        const auto& ifSyntax = prodSyntax.as<syntax::RsIfElseSyntax>();
        const uint64_t id = productionNode(rule, LLG_SLANG_PRODUCTION_IF, 0,
                                           ifSyntax.sourceRange(), "RandSeqIf");
        productionOperand(id, *ifElse.expr, LLG_SLANG_EDGE_CONDITION);
        capture.semanticEdge(id, LLG_SLANG_EDGE_THEN,
                             captureItem(id, ifElse.ifItem, *ifSyntax.ifItem, &context));
        if (ifElse.elseItem && ifSyntax.elseClause)
          capture.semanticEdge(
              id, LLG_SLANG_EDGE_ELSE,
              captureItem(id, *ifElse.elseItem, *ifSyntax.elseClause->item, &context));
        return id;
      }
      case ProdKind::Repeat: {
        const auto& repeat = prod.as<RandSeqProductionSymbol::RepeatProd>();
        const auto& repeatSyntax = prodSyntax.as<syntax::RsRepeatSyntax>();
        const uint64_t id = productionNode(rule, LLG_SLANG_PRODUCTION_REPEAT, 0,
                                           repeatSyntax.sourceRange(), "RandSeqRepeat");
        productionOperand(id, *repeat.expr, LLG_SLANG_EDGE_CONDITION);
        capture.semanticEdge(id, LLG_SLANG_EDGE_BODY,
                             captureItem(id, repeat.item, *repeatSyntax.item, &context));
        return id;
      }
      case ProdKind::Case: {
        const auto& caseProd = prod.as<RandSeqProductionSymbol::CaseProd>();
        const auto& caseSyntax = prodSyntax.as<syntax::RsCaseSyntax>();
        const uint64_t id = productionNode(rule, LLG_SLANG_PRODUCTION_CASE, 0,
                                           caseSyntax.sourceRange(), "RandSeqCase");
        productionOperand(id, *caseProd.expr, LLG_SLANG_EDGE_CASE_EXPRESSION);
        // Items keep syntactic order, so a default item numbers its implicit
        // element where it appears among the standard items.
        uint32_t item = 0;
        bool defaultCaptured = false;
        for (const auto* itemSyntax : caseSyntax.items) {
          if (itemSyntax->kind == syntax::SyntaxKind::DefaultRsCaseItem) {
            // Slang keeps the first of duplicate defaults (an error).
            if (caseProd.defaultItem && !std::exchange(defaultCaptured, true))
              capture.semanticEdge(
                  id, LLG_SLANG_EDGE_ELSE,
                  captureItem(id, *caseProd.defaultItem,
                              *itemSyntax->as<syntax::DefaultRsCaseItemSyntax>().item,
                              &context));
            continue;
          }
          if (item >= caseProd.items.size())
            throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                                "randsequence case items disagree with their syntax");
          const auto& caseItem = caseProd.items[item];
          uint32_t expression = 0;
          for (const Expression* value : caseItem.expressions)
            productionOperand(id, *value, LLG_SLANG_EDGE_CASE_ITEM,
                              (item << 16) | expression++);
          capture.semanticEdge(
              id, LLG_SLANG_EDGE_BRANCH,
              captureItem(id, caseItem.item,
                          *itemSyntax->as<syntax::StandardRsCaseItemSyntax>().item,
                          &context),
              item);
          item++;
        }
        return id;
      }
    }
    throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                        "unknown randsequence production element");
  }

  // A code block's statement scope. Slang (v11) can attach a rule's weight
  // clause code block to the wrong element when the rule also has a code
  // block among its elements, so the block is found by its own syntax.
  uint64_t captureCodeBlock(uint64_t owner, const StatementBlockSymbol& ruleBlock,
                            const syntax::SyntaxNode& codeSyntax) {
    const StatementBlockSymbol* block = nullptr;
    for (const auto& candidate : ruleBlock.membersOfType<StatementBlockSymbol>()) {
      if (candidate.getSyntax() == &codeSyntax) {
        block = &candidate;
        break;
      }
    }
    const Statement* statement = block ? block->tryGetStatement() : nullptr;
    if (!statement)
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "randsequence code block has no bound statement");
    const uint64_t id = productionNode(owner, LLG_SLANG_PRODUCTION_CODE_BLOCK, 0,
                                       codeSyntax.sourceRange(), "RandSeqCodeBlock");
    parents.push_back(id);
    statement->visit(*this);
    block->visit(*this);
    parents.pop_back();
    capture.semanticRole(id, statement, LLG_SLANG_EDGE_BODY);
    return id;
  }

  // One generation of `item.target`: input formal bindings in formal order,
  // then (for a non-void production in a rule) the copy of its result.
  uint64_t captureItem(uint64_t owner, const RandSeqProductionSymbol::ProdItem& item,
                       const syntax::RsProdItemSyntax& itemSyntax, RuleContext* context) {
    const SourceRange range = itemSyntax.sourceRange();
    const uint64_t id =
        productionNode(owner, LLG_SLANG_PRODUCTION_ITEM, 0, range, "RandSeqItem");
    if (!item.target)
      return id;
    const auto& target = *item.target;
    capture.semanticEdge(id, LLG_SLANG_EDGE_CALLEE, capture.ensureSemantic(&target));
    captureBindings(id, target, item.args, range);
    const VariableSymbol* result = productionResult(target);
    if (!context || !result)
      return id;
    const auto found = context->variables.find(target.name);
    if (found == context->variables.end())
      throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                          "randsequence rule has no implicit variable for a production");
    const VariableSymbol& variable = *found->second;
    const int32_t occurrence = ++context->occurrences[&target];
    auto& compilation = target.getCompilation();
    Expression* destination = compilation.emplace<NamedValueExpression>(variable, range);
    const Type& variableType = variable.getType();
    if (variableType.isUnpackedArray()) {
      const auto* element = variableType.getArrayElementType();
      if (!element)
        throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                            "randsequence implicit array has no element type");
      // The literal has no source text of its own, so it carries no location
      // (its spelling would otherwise be the production name).
      const Type& indexType = compilation.getIntType();
      auto* selector = compilation.emplace<IntegerLiteral>(
          compilation, indexType, SVInt(32, uint64_t(occurrence), true), false,
          SourceRange::NoLocation);
      destination = compilation.emplace<ElementSelectExpression>(*element, *destination,
                                                                 *selector, range);
    }
    auto& source = *compilation.emplace<NamedValueExpression>(*result, range);
    auto& copy = *compilation.emplace<AssignmentExpression>(
        std::nullopt, false, *destination->type, *destination, source, nullptr, range);
    productionOperand(id, copy, LLG_SLANG_EDGE_RHS);
    return id;
  }

  void captureBindings(uint64_t id, const RandSeqProductionSymbol& target,
                       std::span<const Expression* const> actuals, SourceRange range) {
    auto& compilation = target.getCompilation();
    for (size_t index = 0; index < target.arguments.size(); index++) {
      const FormalArgumentSymbol& formal = *target.arguments[index];
      if (formal.direction != ArgumentDirection::In || index >= actuals.size() ||
          !actuals[index])
        continue;
      auto& destination = *compilation.emplace<NamedValueExpression>(formal, range);
      auto& binding = *compilation.emplace<AssignmentExpression>(
          std::nullopt, false, formal.getType(), destination,
          const_cast<Expression&>(*actuals[index]), nullptr, range);
      productionOperand(id, binding, LLG_SLANG_EDGE_ARGUMENT, static_cast<uint32_t>(index));
    }
  }

  void captureRandSequence(uint64_t id, const RandSequenceStatement& statement) {
    uint32_t index = 0;
    for (const RandSeqProductionSymbol* production : statement.productions) {
      production->visit(*this);
      capture.semanticRole(id, production, LLG_SLANG_EDGE_DECLARATION, index++);
    }
    if (!statement.firstProduction)
      return;
    // The top production is generated without arguments, so every formal
    // takes its default (Slang already required one).
    const auto& top = *statement.firstProduction;
    const uint64_t item = productionNode(id, LLG_SLANG_PRODUCTION_ITEM, 0,
                                         statement.sourceRange, "RandSeqItem");
    capture.semanticEdge(item, LLG_SLANG_EDGE_CALLEE, capture.ensureSemantic(&top));
    SmallVector<const Expression*> defaults;
    for (const FormalArgumentSymbol* formal : top.arguments)
      defaults.push_back(formal->getDefaultValue());
    captureBindings(item, top,
                    std::span<const Expression* const>(defaults.data(), defaults.size()),
                    statement.sourceRange);
    capture.semanticEdge(id, LLG_SLANG_EDGE_BODY, item);
  }

  std::unordered_map<const RandSeqProductionSymbol*, const VariableSymbol*> productionResults;

  Capture& capture;
  std::vector<uint64_t> parents;
  // Instance bodies and generate blocks being captured, innermost last.
  std::vector<const Scope*> disableScopes;
  std::vector<bool> visited;
  std::unordered_set<const syntax::SyntaxNode*> sourceBodies;
  std::unordered_set<const syntax::SyntaxNode*> sourceGenerateBlocks;
  std::vector<const UninstantiatedDefSymbol*> sourceConnections;
  // The innermost symbol or statement block whose members, statements and
  // expressions are being visited: the lookup context of the names Slang
  // binds and then discards (bindSizeCastNames).
  const Symbol* lookupOwner = nullptr;

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
      // A reference-only target (a method of a class specialization) still
      // says whether it is a static method.
      if (symbol.kind == SymbolKind::Subroutine &&
          symbol.as<SubroutineSymbol>().flags.has(MethodFlags::Static))
        node.auxiliary |= LLG_SLANG_SUBROUTINE_STATIC;
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

  // Typed constraint children (see LLG_SLANG_CONSTRAINT_* in the header):
  // the generic visit attached them as children in visit order.
  template<typename T>
  void addConstraintRoles(uint64_t id, const T& node) {
    if constexpr (std::same_as<T, ConstraintList>) {
      uint32_t index = 0;
      for (const Constraint* item : node.list)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, ExpressionConstraint>) {
      capture.semanticRole(id, &node.expr, LLG_SLANG_EDGE_OPERAND, 0);
    }
    else if constexpr (std::same_as<T, ImplicationConstraint>) {
      capture.semanticRole(id, &node.predicate, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &node.body, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, ConditionalConstraint>) {
      capture.semanticRole(id, &node.predicate, LLG_SLANG_EDGE_CONDITION);
      capture.semanticRole(id, &node.ifBody, LLG_SLANG_EDGE_THEN);
      if (node.elseBody)
        capture.semanticRole(id, node.elseBody, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, UniquenessConstraint>) {
      uint32_t index = 0;
      for (const Expression* item : node.items)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_OPERAND, index++);
    }
    else if constexpr (std::same_as<T, DisableSoftConstraint>) {
      capture.semanticRole(id, &node.target, LLG_SLANG_EDGE_OPERAND, 0);
    }
    else if constexpr (std::same_as<T, SolveBeforeConstraint>) {
      uint32_t index = 0;
      for (const Expression* item : node.solve)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_LEFT, index++);
      index = 0;
      for (const Expression* item : node.after)
        capture.semanticRole(id, item, LLG_SLANG_EDGE_RIGHT, index++);
    }
    else if constexpr (std::same_as<T, ForeachConstraint>) {
      capture.semanticRole(id, &node.arrayRef, LLG_SLANG_EDGE_BASE);
      uint32_t index = 0;
      for (const auto& dimension : node.loopDims) {
        if (dimension.loopVar)
          capture.semanticRole(id, dimension.loopVar, LLG_SLANG_EDGE_DECLARATION,
                               index);
        index++;
      }
      capture.semanticRole(id, &node.body, LLG_SLANG_EDGE_BODY);
    }
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
    capture.semanticChild(parents.back(), id, true);
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

  // A checker instance (IEEE 1800-2009 17.3) is an instance node with the
  // checker subkind; its body is an instance body. The name keeps every
  // declared index of an instance array element, like a module instance.
  void describeCheckerInstance(LlgSlangSemanticNode& result,
                               const CheckerInstanceSymbol& symbol) {
    result.subkind = LLG_SLANG_INSTANCE_CHECKER;
    if (!symbol.arrayPath.empty()) {
      SmallVector<ConstantRange, 8> dimensions;
      symbol.getArrayDimensions(dimensions);
      if (dimensions.size() != symbol.arrayPath.size())
        throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                            "checker array path and dimensions disagree");
      std::string indexedName(symbol.getArrayName());
      for (size_t i = 0; i < dimensions.size(); ++i) {
        if (symbol.arrayPath[i] >= dimensions[i].width())
          throw BridgeFailure(LLG_SLANG_STATUS_INTERNAL_ERROR,
                              "checker array position is outside its dimension");
        const int64_t index = int64_t(dimensions[i].lower()) +
                              int64_t(symbol.arrayPath[i]);
        indexedName += "[" + std::to_string(index) + "]";
      }
      result.name = storeString(capture.output, indexedName);
    }
    const CheckerInstanceBodySymbol& body = symbol.body;
    result.definition_name = storeString(capture.output, body.checker.name);
    if (body.isProcedural)
      result.auxiliary |= LLG_SLANG_CHECKER_PROCEDURAL;
    if (body.flags.has(InstanceFlags::FromBind))
      result.auxiliary |= LLG_SLANG_CHECKER_BOUND;
    if (body.instanceDepth > 0)
      result.auxiliary |= LLG_SLANG_CHECKER_NESTED;
    if (body.flags.has(InstanceFlags::Uninstantiated))
      result.flags |= LLG_SLANG_SEMANTIC_UNINSTANTIATED;
    // A checker has no time scale of its own: it takes the scale of the
    // scope that declares it, like its other declaration context (17.2).
    // Slang reports the compilation default for a compilation unit without
    // a time unit declaration; the consumer applies its `$unit` default.
    if (const Scope* declaring = body.checker.getParentScope()) {
      const Symbol& scope = declaring->asSymbol();
      if (scope.kind == SymbolKind::CompilationUnit &&
          !compilationUnitDeclaresTimeScale(scope.as<CompilationUnitSymbol>()))
        result.auxiliary |= LLG_SLANG_CHECKER_UNIT_TIME_SCALE;
      else
        addTimeScale(result, declaring->getTimeScale());
    }
  }

  // Formal references of the checker instances being captured. Slang binds
  // a reference to a checker formal by binding the actual argument (or the
  // formal's default) in its own context and stamping the reference's
  // syntax on the result (AssertionInstanceExpression::bindPort and
  // Expression::create). The body expressions a formal reference expanded
  // to are therefore those whose syntax is one of the declaration's
  // identifier names spelling a formal, unless the name resolves to a
  // checker-local declaration that shadows the formal.
  struct CheckerActuals {
    uint64_t instance;
    const CheckerInstanceBodySymbol* body;
    // Identifier syntax in the checker declaration spelling a formal name,
    // and the syntax of each formal's default value (second: true).
    const std::unordered_map<const syntax::SyntaxNode*,
                             std::pair<std::string_view, bool>>* references;
    uint32_t next = 0;
  };
  std::vector<CheckerActuals> checkerActuals;
  std::unordered_map<const CheckerSymbol*,
                     std::unordered_map<const syntax::SyntaxNode*,
                                        std::pair<std::string_view, bool>>>
      checkerReferences;

  const std::unordered_map<const syntax::SyntaxNode*, std::pair<std::string_view, bool>>&
  checkerFormalReferences(const CheckerSymbol& checker) {
    auto [it, inserted] = checkerReferences.try_emplace(&checker);
    if (!inserted)
      return it->second;
    std::unordered_set<std::string_view> names;
    for (const AssertionPortSymbol* port : checker.ports) {
      if (!port->name.empty())
        names.insert(port->name);
    }
    const syntax::SyntaxNode* declaration = checker.getSyntax();
    if (names.empty() || !declaration ||
        declaration->kind != syntax::SyntaxKind::CheckerDeclaration)
      return it->second;
    auto& references = it->second;
    // A default value's own expression keeps its syntax where the expansion
    // is not wrapped (an event formal's clocking event, for example).
    for (const AssertionPortSymbol* port : checker.ports) {
      if (const auto* value = checkerActualExpression(port->defaultValueSyntax))
        references.emplace(value, std::make_pair(port->name, true));
    }
    auto visitor = syntax::makeSyntaxVisitor(
        [&](auto&, const syntax::IdentifierNameSyntax& name) {
          const std::string_view text = name.identifier.valueText();
          if (const auto found = names.find(text); found != names.end())
            references.emplace(&name, std::make_pair(*found, false));
        });
    for (const syntax::MemberSyntax* member :
         declaration->as<syntax::CheckerDeclarationSyntax>().members)
      member->visit(visitor);
    return references;
  }

  // Expansion aliases of the let, sequence and property instances being
  // captured inside a checker, innermost last. Slang binds a reference to a
  // formal of such a declaration by binding the actual argument and stamping
  // the reference's syntax on the result, so an expanded checker formal
  // carries the syntax of the declaration's own formal reference, which may
  // lie outside the checker. Each alias maps a formal reference in the
  // declaration to the actual argument syntax of this instance.
  std::vector<std::unordered_map<const syntax::SyntaxNode*, const syntax::SyntaxNode*>>
      assertionAliases;
  // Per let, sequence or property declaration: the identifier names in it
  // that spell one of its formals (`formalReferences`).
  std::unordered_map<const void*,
                     std::unordered_map<const syntax::SyntaxNode*, std::string_view>>
      assertionFormalReferences;
  // Syntax of the expressions being captured inside a checker, innermost
  // last, and the let expansions matched so far per (checker instance,
  // calls scope, let).
  std::vector<const syntax::SyntaxNode*> captureSyntax;
  std::map<std::tuple<uint64_t, const syntax::SyntaxNode*, const syntax::SyntaxNode*>, size_t>
      letExpansionCounts;

  // Pushes the aliases of `expression` and returns whether it pushed any.
  bool pushAssertionAliases(const AssertionInstanceExpression& expression) {
    const syntax::SyntaxNode* declaration = expression.symbol.getSyntax();
    if (!declaration || expression.arguments.empty())
      return false;
    std::unordered_set<std::string_view> names;
    for (const auto& [formal, actual] : expression.arguments) {
      (void)actual;
      if (formal && !formal->name.empty())
        names.insert(formal->name);
    }
    std::unordered_map<const syntax::SyntaxNode*, const syntax::SyntaxNode*> aliases;
    for (const auto& [reference, name] :
         formalReferences(&expression.symbol, *declaration, names)) {
      for (const auto& [formal, actual] : expression.arguments) {
        const Expression* const* value = std::get_if<const Expression*>(&actual);
        if (formal && formal->name == name && value && *value && (*value)->syntax)
          aliases.emplace(reference, (*value)->syntax);
      }
    }
    if (aliases.empty())
      return false;
    assertionAliases.push_back(std::move(aliases));
    return true;
  }

  // Identifier names in `root` that spell one of `names`, cached per
  // declaration.
  const std::unordered_map<const syntax::SyntaxNode*, std::string_view>&
  formalReferences(const void* declaration, const syntax::SyntaxNode& root,
                   const std::unordered_set<std::string_view>& names) {
    auto [it, inserted] = assertionFormalReferences.try_emplace(declaration);
    if (inserted) {
      auto& references = it->second;
      auto visitor = syntax::makeSyntaxVisitor(
          [&](auto&, const syntax::IdentifierNameSyntax& name) {
            const std::string_view text = name.identifier.valueText();
            if (const auto found = names.find(text); found != names.end())
              references.emplace(&name, *found);
          });
      root.visit(visitor);
    }
    return it->second;
  }

  // Slang expands a let instance directly to the let's expression (11.13):
  // the expansion keeps the syntax of the let declaration and no link to
  // the invocation. Its root is the first captured expression whose syntax
  // lies in a let declaration while the enclosing captured expression's
  // does not. The invocation is a call of that let in the enclosing
  // expression's syntax or, at the root of a statement or declaration, in
  // the checker declaration; a let whose whole expression is a call of
  // another let expands to that let, so a call of the outer let also
  // matches, through the chain. Matching calls are taken in source order.
  // Pushes the aliases of each let of the chosen chain, outermost first,
  // and returns how many frames it pushed.
  size_t pushLetAliases(const Expression& expression) {
    if (!expression.syntax)
      return 0;
    const syntax::LetDeclarationSyntax* let = nullptr;
    for (const syntax::SyntaxNode* node = expression.syntax->parent; node; node = node->parent) {
      if (node->kind == syntax::SyntaxKind::LetDeclaration) {
        let = &node->as<syntax::LetDeclarationSyntax>();
        break;
      }
    }
    if (!let)
      return 0;
    const syntax::SyntaxNode* enclosing =
        captureSyntax.empty() ? nullptr : captureSyntax.back();
    for (const syntax::SyntaxNode* node = enclosing; node; node = node->parent) {
      if (node == let)
        return 0;
    }
    std::vector<LetChain> candidates;
    const syntax::SyntaxNode* scope = enclosing;
    if (enclosing)
      collectLetChains(*enclosing, *let, candidates);
    if (candidates.empty()) {
      scope = checkerActuals.back().body->checker.getSyntax();
      if (scope)
        collectLetChains(*scope, *let, candidates);
    }
    if (candidates.empty())
      return 0;
    size_t& next = letExpansionCounts[{checkerActuals.back().instance, scope, let}];
    const LetChain& chain = candidates[next % candidates.size()];
    ++next;
    size_t pushed = 0;
    for (const auto& [declaration, invocation] : chain)
      pushed += pushInvocationAliases(*declaration, *invocation) ? 1 : 0;
    return pushed;
  }

  // The lets of one expansion, outermost first, each with its call.
  using LetChain = std::vector<
      std::pair<const syntax::LetDeclarationSyntax*, const syntax::InvocationExpressionSyntax*>>;

  // Maximum number of lets one chain may pass through: each step is a let
  // whose whole expression calls the next.
  static constexpr size_t LetChainDepth = 16;

  // The calls in `root`, in source order, that expand to `target`: calls of
  // `target` itself and calls of lets whose whole expression is (through
  // further such lets) a call of `target`.
  void collectLetChains(const syntax::SyntaxNode& root, const syntax::LetDeclarationSyntax& target,
                        std::vector<LetChain>& out) {
    std::vector<const syntax::InvocationExpressionSyntax*> calls;
    auto visitor = syntax::makeSyntaxVisitor(
        [&](auto& self, const syntax::InvocationExpressionSyntax& call) {
          if (call.left->kind == syntax::SyntaxKind::IdentifierName)
            calls.push_back(&call);
          self.visitDefault(call);
        });
    root.visit(visitor);
    const std::string_view targetName = target.identifier.valueText();
    for (const syntax::InvocationExpressionSyntax* call : calls) {
      LetChain chain;
      const syntax::InvocationExpressionSyntax* current = call;
      for (size_t depth = 0; current && depth < LetChainDepth; ++depth) {
        const std::string_view name =
            current->left->as<syntax::IdentifierNameSyntax>().identifier.valueText();
        if (name == targetName) {
          chain.emplace_back(&target, current);
          out.push_back(std::move(chain));
          break;
        }
        const syntax::LetDeclarationSyntax* declaration = letDeclaration(name);
        if (!declaration)
          break;
        chain.emplace_back(declaration, current);
        const syntax::ExpressionSyntax* body = declaration->expr;
        while (body->kind == syntax::SyntaxKind::ParenthesizedExpression)
          body = body->as<syntax::ParenthesizedExpressionSyntax>().expression;
        current = body->kind == syntax::SyntaxKind::InvocationExpression &&
                          body->as<syntax::InvocationExpressionSyntax>().left->kind ==
                              syntax::SyntaxKind::IdentifierName
                      ? &body->as<syntax::InvocationExpressionSyntax>()
                      : nullptr;
      }
    }
  }

  // The let declaration `name` resolves to from the innermost checker body.
  const syntax::LetDeclarationSyntax* letDeclaration(std::string_view name) {
    const Symbol* symbol = checkerActuals.back().body->lookupName(name);
    if (!symbol || symbol->kind != SymbolKind::LetDecl)
      return nullptr;
    const syntax::SyntaxNode* declaration = symbol->getSyntax();
    return declaration && declaration->kind == syntax::SyntaxKind::LetDeclaration
               ? &declaration->as<syntax::LetDeclarationSyntax>()
               : nullptr;
  }

  // Pushes the aliases of the formals of `let` at call `invocation` and
  // returns whether it pushed any.
  bool pushInvocationAliases(const syntax::LetDeclarationSyntax& let,
                             const syntax::InvocationExpressionSyntax& invocation) {
    std::unordered_map<std::string_view, const syntax::SyntaxNode*> actuals;
    std::unordered_set<std::string_view> names;
    std::vector<std::string_view> order;
    if (let.portList) {
      for (const syntax::AssertionItemPortSyntax* port : let.portList->ports) {
        const std::string_view portName = port->name.valueText();
        order.push_back(portName);
        if (portName.empty())
          continue;
        names.insert(portName);
        if (port->defaultValue) {
          if (const auto* value = checkerActualExpression(port->defaultValue->expr))
            actuals[portName] = value;
        }
      }
    }
    if (invocation.arguments) {
      size_t ordered = 0;
      for (const syntax::ArgumentSyntax* argument : invocation.arguments->parameters) {
        if (argument->kind == syntax::SyntaxKind::OrderedArgument) {
          if (ordered < order.size()) {
            if (const auto* value = checkerActualExpression(
                    argument->as<syntax::OrderedArgumentSyntax>().expr))
              actuals[order[ordered]] = value;
          }
          ordered++;
        }
        else if (argument->kind == syntax::SyntaxKind::NamedArgument) {
          const auto& named = argument->as<syntax::NamedArgumentSyntax>();
          if (const auto* value = checkerActualExpression(named.expr))
            actuals[named.name.valueText()] = value;
        }
      }
    }
    std::unordered_map<const syntax::SyntaxNode*, const syntax::SyntaxNode*> aliases;
    for (const auto& [reference, portName] : formalReferences(&let, *let.expr, names)) {
      if (const auto found = actuals.find(portName); found != actuals.end())
        aliases.emplace(reference, found->second);
    }
    if (aliases.empty())
      return false;
    assertionAliases.push_back(std::move(aliases));
    return true;
  }

  // The actual argument syntax that `reference` expands to through the let,
  // sequence and property instances being captured, or `reference` itself.
  const syntax::SyntaxNode* resolveAssertionAlias(const syntax::SyntaxNode* reference) const {
    // Each step moves to an enclosing instance's argument, so the chain is
    // no longer than the stack.
    for (size_t step = 0; step < assertionAliases.size(); ++step) {
      bool moved = false;
      for (auto frame = assertionAliases.rbegin(); frame != assertionAliases.rend(); ++frame) {
        if (const auto found = frame->find(reference); found != frame->end()) {
          reference = found->second;
          moved = true;
          break;
        }
      }
      if (!moved)
        break;
    }
    return reference;
  }

  void pushCheckerActuals(uint64_t id, const CheckerInstanceSymbol& symbol) {
    checkerActuals.push_back(
        {id, &symbol.body, &checkerFormalReferences(symbol.body.checker), 0});
  }

  static bool declaredWithin(const Symbol& symbol, const Scope& scope) {
    for (const Scope* current = symbol.getParentScope(); current;
         current = current->asSymbol().getParentScope()) {
      if (current == &scope)
        return true;
    }
    return false;
  }

  static const syntax::ExpressionSyntax* checkerActualExpression(
      const syntax::PropertyExprSyntax* actual) {
    if (!actual || actual->kind != syntax::SyntaxKind::SimplePropertyExpr)
      return nullptr;
    const syntax::SequenceExprSyntax* sequence =
        actual->as<syntax::SimplePropertyExprSyntax>().expr;
    if (sequence->kind != syntax::SyntaxKind::SimpleSequenceExpr)
      return nullptr;
    const auto& simple = sequence->as<syntax::SimpleSequenceExprSyntax>();
    if (simple.repetition)
      return nullptr;
    const syntax::ExpressionSyntax* expression = simple.expr.get();
    while (expression->kind == syntax::SyntaxKind::ParenthesizedExpression)
      expression = expression->as<syntax::ParenthesizedExpressionSyntax>().expression;
    return expression;
  }

  // Records `id` as the expansion of a checker formal (17.3). A formal of a
  // checker instantiated in another checker is connected directly, not
  // sampled (17.3.2): when its actual is itself the parent's formal, the
  // expansion belongs to the parent's formal, otherwise it is not recorded.
  // A default value is recorded at the instance that uses it.
  void markCheckerActual(uint64_t id, const Expression& expression) {
    if (!expression.syntax)
      return;
    const syntax::SyntaxNode* reference = resolveAssertionAlias(expression.syntax);
    size_t level = checkerActuals.size();
    while (level-- > 0) {
      CheckerActuals& entry = checkerActuals[level];
      const auto found = entry.references->find(reference);
      if (found == entry.references->end())
        continue;
      const CheckerInstanceBodySymbol& body = *entry.body;
      if (const Symbol* target = expression.getSymbolReference();
          target && declaredWithin(*target, body))
        return;
      const auto [name, defaultSyntax] = found->second;
      const AssertionPortSymbol* port = nullptr;
      for (const AssertionPortSymbol* candidate : body.checker.ports) {
        if (candidate->name == name)
          port = candidate;
      }
      const Symbol* formal = body.find(name);
      if (!port || !formal)
        return;
      const auto binding = body.assertionDetails.argumentMap.find(formal);
      if (binding == body.assertionDetails.argumentMap.end())
        return;
      const syntax::PropertyExprSyntax* actual = std::get<0>(binding->second);
      const bool isDefault = actual && actual == port->defaultValueSyntax;
      if (defaultSyntax && !isDefault)
        continue;
      if (!isDefault && body.instanceDepth > 0) {
        const syntax::ExpressionSyntax* forwarded = checkerActualExpression(actual);
        if (!forwarded || forwarded->kind != syntax::SyntaxKind::IdentifierName ||
            level == 0)
          return;
        reference = forwarded;
        continue;
      }
      const uint32_t flag = isDefault ? LLG_SLANG_CHECKER_ACTUAL_DEFAULT : 0;
      capture.semanticEdge(entry.instance, LLG_SLANG_EDGE_CHECKER_ACTUAL, id,
                           entry.next++ | flag);
      // An event formal in an event control expands to the actual's
      // clocking event; its event expressions are what consumers resolve.
      if (expression.kind == ExpressionKind::ClockingEvent)
        markCheckerEvents(entry, expression.as<ClockingEventExpression>().timingControl,
                          flag);
      return;
    }
  }

  void markCheckerEvents(CheckerActuals& entry, const TimingControl& timing, uint32_t flag) {
    if (timing.kind == TimingControlKind::SignalEvent) {
      capture.semanticEdge(entry.instance, LLG_SLANG_EDGE_CHECKER_ACTUAL,
                           capture.ensureSemantic(&timing.as<SignalEventControl>().expr),
                           entry.next++ | flag);
    }
    else if (timing.kind == TimingControlKind::EventList) {
      for (const TimingControl* event : timing.as<EventListControl>().events)
        markCheckerEvents(entry, *event, flag);
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

  // Functional-coverage metadata (IEEE 1800-2009 19). Only typed flags are
  // set here; edges that need captured targets are added by
  // captureCoverageRoles once the members and expressions were visited.
  template<typename T>
  void captureCoverageSymbol(uint64_t id, const T& symbol) {
    uint32_t subkind = 0;
    uint64_t auxiliary = 0;
    uint64_t typeId = LLG_SLANG_INVALID_ID;
    if constexpr (std::same_as<T, CovergroupType>) {
      subkind = LLG_SLANG_COVERAGE_GROUP;
      typeId = capture.type(symbol);
      const Scope* parent = symbol.getParentScope();
      if (parent && parent->asSymbol().kind == SymbolKind::ClassType)
        auxiliary |= LLG_SLANG_COVERGROUP_EMBEDDED;
      if (const auto* syntaxNode = symbol.getSyntax()) {
        const auto& declaration =
            syntaxNode->template as<syntax::CovergroupDeclarationSyntax>();
        if (declaration.extends)
          auxiliary |= LLG_SLANG_COVERGROUP_EXTENDS;
        if (declaration.event) {
          if (declaration.event->kind == syntax::SyntaxKind::WithFunctionSample)
            auxiliary |= LLG_SLANG_COVERGROUP_WITH_SAMPLE;
          else if (declaration.event->kind == syntax::SyntaxKind::BlockCoverageEvent)
            auxiliary |= LLG_SLANG_COVERGROUP_BLOCK_EVENT;
        }
      }
    }
    if constexpr (std::same_as<T, CovergroupBodySymbol>)
      subkind = LLG_SLANG_COVERAGE_BODY;
    if constexpr (std::same_as<T, CoverpointSymbol>) {
      subkind = LLG_SLANG_COVERAGE_POINT;
      typeId = capture.type(symbol.getType());
    }
    if constexpr (std::same_as<T, CoverageBinSymbol>) {
      subkind = LLG_SLANG_COVERAGE_BIN;
      switch (symbol.binsKind) {
        case CoverageBinSymbol::Bins: auxiliary = LLG_SLANG_COVERAGE_BIN_BINS; break;
        case CoverageBinSymbol::IllegalBins:
          auxiliary = LLG_SLANG_COVERAGE_BIN_ILLEGAL;
          break;
        case CoverageBinSymbol::IgnoreBins:
          auxiliary = LLG_SLANG_COVERAGE_BIN_IGNORE;
          break;
      }
      if (symbol.isArray)
        auxiliary |= LLG_SLANG_COVERAGE_BIN_ARRAY;
      if (symbol.isWildcard)
        auxiliary |= LLG_SLANG_COVERAGE_BIN_WILDCARD;
      if (symbol.isDefault)
        auxiliary |= LLG_SLANG_COVERAGE_BIN_DEFAULT;
      if (symbol.isDefaultSequence)
        auxiliary |= LLG_SLANG_COVERAGE_BIN_DEFAULT_SEQUENCE;
      if (!symbol.getTransList().empty())
        auxiliary |= LLG_SLANG_COVERAGE_BIN_TRANSITION;
      if (symbol.getWithExpr())
        auxiliary |= LLG_SLANG_COVERAGE_BIN_WITH;
      if (symbol.getSetCoverageExpr())
        auxiliary |= LLG_SLANG_COVERAGE_BIN_SET;
      if (symbol.getCrossSelectExpr())
        auxiliary |= LLG_SLANG_COVERAGE_BIN_SELECT;
    }
    if constexpr (std::same_as<T, CoverCrossSymbol>) {
      subkind = LLG_SLANG_COVERAGE_CROSS;
      // IEEE 1800-2009 A.2.11 crosses coverpoints and variables only; the
      // frontend expands a crossed cross into its coverpoints.
      const Scope* parent = symbol.getParentScope();
      if (const auto* syntaxNode = symbol.getSyntax(); syntaxNode && parent) {
        for (const auto* item :
             syntaxNode->template as<syntax::CoverCrossSyntax>().items) {
          if (item->kind != syntax::SyntaxKind::IdentifierName)
            continue;
          const Symbol* found = parent->find(
              item->template as<syntax::IdentifierNameSyntax>().identifier.valueText());
          if (found && found->kind == SymbolKind::CoverCross)
            auxiliary |= LLG_SLANG_COVER_CROSS_OF_CROSS;
        }
      }
    }
    auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
    node.subkind = subkind;
    node.auxiliary = auxiliary;
    if (typeId != LLG_SLANG_INVALID_ID)
      node.type_id = typeId;
  }

  template<typename T>
  void captureCoverageRoles(uint64_t id, const T& symbol) {
    if (capture.declarationOnly)
      return;
    if constexpr (std::same_as<T, CovergroupType>) {
      uint32_t index = 0;
      for (const FormalArgumentSymbol* argument : symbol.getArguments())
        capture.semanticRole(id, argument, LLG_SLANG_EDGE_ARGUMENT, index++);
      index = 0;
      for (const Symbol& member : symbol.members()) {
        if (member.kind == SymbolKind::FormalArgument &&
            member.template as<FormalArgumentSymbol>().flags.has(
                VariableFlags::CoverageSampleFormal))
          capture.semanticRole(id, &member, LLG_SLANG_EDGE_DECLARATION, index++);
      }
      if (const TimingControl* event = symbol.getCoverageEvent())
        capture.semanticRole(id, event, LLG_SLANG_EDGE_EVENT);
      const CovergroupBodySymbol& body = symbol.getBody();
      index = 0;
      for (const CoverageOptionSetter& option : body.options) {
        const Expression& expression = option.getExpression();
        expression.visit(*this);
        capture.semanticRole(id, &expression, LLG_SLANG_EDGE_OPERAND, index++);
      }
      capture.semanticRole(id, &body, LLG_SLANG_EDGE_BODY);
    }
    if constexpr (std::same_as<T, CoverpointSymbol>) {
      capture.semanticRole(id, &symbol.getCoverageExpr(), LLG_SLANG_EDGE_INITIALIZER);
      if (const Expression* iff = symbol.getIffExpr())
        capture.semanticRole(id, iff, LLG_SLANG_EDGE_CONDITION);
      uint32_t index = 0;
      for (const CoverageOptionSetter& option : symbol.options)
        capture.semanticRole(id, &option.getExpression(), LLG_SLANG_EDGE_OPERAND, index++);
    }
    if constexpr (std::same_as<T, CoverageBinSymbol>) {
      uint32_t index = 0;
      for (const Expression* value : symbol.getValues())
        capture.semanticRole(id, value, LLG_SLANG_EDGE_OPERAND, index++);
      // Transition sets and their range lists have no symbol identity; each
      // becomes a synthetic node owned by the bin. Their items were visited
      // with the bin's expressions and stay its children.
      for (const auto& set : symbol.getTransList()) {
        const uint64_t setId = coverageNode(id, LLG_SLANG_COVERAGE_TRANS_SET, 0);
        capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, setId, index++);
        uint32_t step = 0;
        for (const auto& range : set) {
          uint64_t repeat = LLG_SLANG_COVER_REPEAT_NONE;
          switch (range.repeatKind) {
            case CoverageBinSymbol::TransRangeList::None: break;
            case CoverageBinSymbol::TransRangeList::Consecutive:
              repeat = LLG_SLANG_COVER_REPEAT_CONSECUTIVE;
              break;
            case CoverageBinSymbol::TransRangeList::Nonconsecutive:
              repeat = LLG_SLANG_COVER_REPEAT_NONCONSECUTIVE;
              break;
            case CoverageBinSymbol::TransRangeList::GoTo:
              repeat = LLG_SLANG_COVER_REPEAT_GOTO;
              break;
          }
          const uint64_t rangeId =
              coverageNode(id, LLG_SLANG_COVERAGE_TRANS_RANGE, repeat);
          capture.semanticEdge(setId, LLG_SLANG_EDGE_OPERAND, rangeId, step++);
          uint32_t item = 0;
          for (const Expression* value : range.items)
            capture.semanticEdge(rangeId, LLG_SLANG_EDGE_OPERAND,
                                 capture.ensureSemantic(value), item++);
          if (range.repeatFrom)
            capture.semanticEdge(rangeId, LLG_SLANG_EDGE_LEFT,
                                 capture.ensureSemantic(range.repeatFrom));
          if (range.repeatTo)
            capture.semanticEdge(rangeId, LLG_SLANG_EDGE_RIGHT,
                                 capture.ensureSemantic(range.repeatTo));
        }
      }
      if (const Expression* count = symbol.getNumberOfBinsExpr())
        capture.semanticRole(id, count, LLG_SLANG_EDGE_WIDTH);
      if (const Expression* iff = symbol.getIffExpr())
        capture.semanticRole(id, iff, LLG_SLANG_EDGE_CONDITION);
      if (const BinsSelectExpr* select = symbol.getCrossSelectExpr())
        capture.semanticEdge(id, LLG_SLANG_EDGE_BODY, captureSelect(id, *select));
    }
    if constexpr (std::same_as<T, CoverCrossSymbol>) {
      uint32_t index = 0;
      for (const CoverpointSymbol* target : symbol.targets)
        capture.semanticEdge(id, LLG_SLANG_EDGE_ARGUMENT,
                             capture.ensureSemantic(target), index++);
      if (const Expression* iff = symbol.getIffExpr())
        capture.semanticRole(id, iff, LLG_SLANG_EDGE_CONDITION);
      index = 0;
      for (const CoverageOptionSetter& option : symbol.options)
        capture.semanticRole(id, &option.getExpression(), LLG_SLANG_EDGE_OPERAND, index++);
    }
  }

  // A synthetic coverage node (transition set or range list) owned by `bin`.
  uint64_t coverageNode(uint64_t bin, uint32_t subkind, uint64_t auxiliary) {
    const uint64_t id = capture.newSyntheticSemantic();
    const auto& owner = capture.output.semantic_nodes[static_cast<size_t>(bin)];
    const LlgSlangSourceRange range = owner.range;
    auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
    node.parent_id = bin;
    node.kind = LLG_SLANG_SEMANTIC_COVERAGE;
    node.subkind = subkind;
    node.auxiliary = auxiliary;
    node.range = range;
    node.detail = storeString(capture.output, subkind == LLG_SLANG_COVERAGE_TRANS_SET
                                                  ? "TransSet"
                                                  : "TransRangeList");
    return id;
  }

  // One cross bin select expression (19.6): a SELECT node per operator. The
  // intersect items were visited with the bin's expressions.
  uint64_t captureSelect(uint64_t bin, const BinsSelectExpr& select) {
    const uint64_t id = capture.ensureSemantic(&select);
    uint64_t kind = LLG_SLANG_COVER_SELECT_OTHER;
    switch (select.kind) {
      case BinsSelectExprKind::Condition: kind = LLG_SLANG_COVER_SELECT_CONDITION; break;
      case BinsSelectExprKind::Unary: kind = LLG_SLANG_COVER_SELECT_NOT; break;
      case BinsSelectExprKind::Binary:
        kind = select.as<BinaryBinsSelectExpr>().op == BinaryBinsSelectExpr::And
                   ? LLG_SLANG_COVER_SELECT_AND
                   : LLG_SLANG_COVER_SELECT_OR;
        break;
      default: break;
    }
    {
      const auto& owner = capture.output.semantic_nodes[static_cast<size_t>(bin)];
      const LlgSlangSourceRange range = select.syntax
                                            ? capture.span(select.syntax->sourceRange())
                                            : owner.range;
      auto& node = capture.output.semantic_nodes[static_cast<size_t>(id)];
      node.parent_id = bin;
      node.kind = LLG_SLANG_SEMANTIC_COVERAGE;
      node.subkind = LLG_SLANG_COVERAGE_SELECT;
      node.auxiliary = kind;
      node.range = range;
      node.detail = storeString(capture.output, toString(select.kind));
    }
    if (const auto* condition = select.as_if<ConditionBinsSelectExpr>()) {
      capture.semanticEdge(id, LLG_SLANG_EDGE_REFERENCE,
                           captureReferenceTarget(condition->target));
      uint32_t index = 0;
      for (const Expression* item : condition->intersects)
        capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND,
                             capture.ensureSemantic(item), index++);
    }
    else if (const auto* unary = select.as_if<UnaryBinsSelectExpr>()) {
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, captureSelect(bin, unary->expr), 0);
    }
    else if (const auto* binary = select.as_if<BinaryBinsSelectExpr>()) {
      const uint64_t left = captureSelect(bin, binary->left);
      const uint64_t right = captureSelect(bin, binary->right);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, left, 0);
      capture.semanticEdge(id, LLG_SLANG_EDGE_OPERAND, right, 1);
    }
    return id;
  }

  template<std::derived_from<Symbol> T>
  void addSymbolRoles(const T& symbol, uint64_t id) {
    if constexpr (std::derived_from<T, ValueSymbol>) {
      if (const Expression* initializer = symbol.getInitializer())
        capture.semanticRole(id, initializer, LLG_SLANG_EDGE_INITIALIZER);
    }
    if constexpr (std::same_as<T, InstanceBodySymbol> ||
                  std::same_as<T, CheckerInstanceBodySymbol>) {
      // The default clocking in effect for this instance, including one named
      // by a `default clocking id;` item (IEEE 1800-2009 14.12). Slang keeps
      // that reference only in its compilation; the edge is appended, never
      // converted from the block's own child edge.
      if (const Symbol* clocking = symbol.getCompilation().getDefaultClocking(symbol))
        capture.semanticEdge(id, LLG_SLANG_EDGE_CLOCKING,
                             capture.ensureSemantic(clocking));
    }
    if constexpr (std::same_as<T, InstanceBodySymbol>) {
      // The `default disable iff` declared in this instance itself (ABI
      // v62), its vpiDefaultDisableIff (IEEE 1800-2009 37.5, 37.6, 37.9).
      // Slang keeps it only in its compilation, keyed by the declaring
      // scope; enclosing scopes do not count.
      const auto& disables = symbol.getCompilation().getDefaultDisableMap();
      if (auto it = disables.find(static_cast<const Scope*>(&symbol));
          it != disables.end()) {
        parents.push_back(id);
        it->second->visit(*this);
        parents.pop_back();
        capture.semanticRole(id, it->second, LLG_SLANG_EDGE_DEFAULT_DISABLE);
      }
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
    // A multi-port connects internally through its component ports, most
    // significant first; it has no internal symbol or connection of its own.
    if constexpr (std::same_as<T, MultiPortSymbol>) {
      uint32_t index = 0;
      for (const PortSymbol* component : symbol.ports)
        capture.semanticRole(id, component, LLG_SLANG_EDGE_LOW_CONNECTION, index++);
    }
    if constexpr (std::same_as<T, SubroutineSymbol>) {
      const Statement& body = symbol.getBody();
      capture.semanticRole(id, &body, LLG_SLANG_EDGE_BODY);
      // A body of several statements is a list without a source range:
      // keep where it opens (the token after the name), in the file of the
      // subroutine.
      const syntax::SyntaxNode* syntax = symbol.getSyntax();
      if (syntax && syntax->kind == syntax::SyntaxKind::ClassMethodDeclaration)
        syntax = syntax->as<syntax::ClassMethodDeclarationSyntax>().declaration;
      if (body.kind == StatementKind::List && syntax &&
          syntax::FunctionDeclarationSyntax::isKind(syntax->kind)) {
        const auto& declaration = syntax->as<syntax::FunctionDeclarationSyntax>();
        const uint64_t file =
            capture.output.semantic_nodes[static_cast<size_t>(id)].range.file_id;
        const parsing::Token open = declaration.prototype->portList
                               ? declaration.prototype->portList->openParen
                               : declaration.semi;
        if (const uint64_t begin = capture.keywordOffset(open.location(), file)) {
          const uint64_t bodyId = capture.ensureSemantic(&body);
          capture.output.semantic_nodes[static_cast<size_t>(bodyId)].auxiliary = begin;
        }
      }
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
      // A declaration assignment's delay belongs to that continuous
      // assignment and is not a net delay (IEEE 1364-2001 6.1.3, IEEE
      // 1800-2009 10.3.3), so it never delays the net's other drivers.
      else if (const TimingControl* delay = symbol.getDelay())
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
    // Slang expands the connection of a multi-port (a concatenated port
    // expression, IEEE 1800-2009 23.2.2.1) into one connection per component
    // port, so the expanded list is longer than the port list. Each port-list
    // entry owns `count` consecutive expanded connections.
    struct ConnectionGroup {
      const Symbol* port;
      size_t first;
      size_t count;
    };
    std::vector<ConnectionGroup> groups;
    size_t expanded = 0;
    for (const Symbol* port : symbol.body.getPortList()) {
      const size_t count = port->kind == SymbolKind::MultiPort
          ? port->as<MultiPortSymbol>().ports.size()
          : 1;
      groups.push_back({port, expanded, count});
      expanded += count;
    }
    // Marks the group's port and, for a multi-port, each component as
    // connected; a component Slang left without an actual (the actual is
    // narrower than the port) is an open connection. Returns the node of the
    // port named in the instance syntax.
    auto markGroup = [&](const ConnectionGroup& group, bool referenceTarget) {
      auto portNode = [&](const Symbol& port) {
        return referenceTarget && capture.declarationOnly
            ? captureReferenceTarget(port)
            : capture.ensureSemantic(&port);
      };
      bool anyActual = false;
      for (size_t index = group.first;
           index < group.first + group.count && index < semanticConnections.size();
           index++) {
        const PortConnection* connection = semanticConnections[index];
        auto& node = capture.output.semantic_nodes[static_cast<size_t>(
            portNode(connection->port))];
        node.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_PRESENT;
        if (connection->getExpression())
          anyActual = true;
        else
          node.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_OPEN;
      }
      const uint64_t portId = portNode(*group.port);
      auto& node = capture.output.semantic_nodes[static_cast<size_t>(portId)];
      node.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_PRESENT;
      if (!anyActual)
        node.flags |= LLG_SLANG_SEMANTIC_PORT_CONNECTION_OPEN;
      return portId;
    };
    // Marks the ports of a group as connected by name.
    auto markByName = [&](const ConnectionGroup& group) {
      for (size_t index = group.first;
           index < group.first + group.count && index < semanticConnections.size();
           index++)
        capture.output.semantic_nodes[static_cast<size_t>(capture.ensureSemantic(
            &semanticConnections[index]->port))].auxiliary |=
            LLG_SLANG_PORT_CONNECTED_BY_NAME;
      capture.output.semantic_nodes[static_cast<size_t>(
          capture.ensureSemantic(group.port))].auxiliary |=
          LLG_SLANG_PORT_CONNECTED_BY_NAME;
    };
    std::vector<bool> namedGroups(groups.size(), false);
    bool wildcard = false;
    uint32_t orderedIndex = 0;
    std::unordered_set<std::string_view> namedSeen;
    for (const syntax::PortConnectionSyntax* syntaxConnection :
         instanceSyntax.connections) {
      if (syntaxConnection->kind != syntax::SyntaxKind::NamedPortConnection) {
        if (syntaxConnection->kind == syntax::SyntaxKind::WildcardPortConnection) {
          wildcard = true;
          continue;
        }
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
        if (orderedIndex < groups.size())
          markGroup(groups[orderedIndex], false);
        orderedIndex++;
        continue;
      }
      const auto& named = syntaxConnection->as<syntax::NamedPortConnectionSyntax>();
      if (named.expr)
        capture.connectionActual(named.expr->sourceRange());
      const std::string_view name = named.name.valueText();
      bool connected = false;
      for (size_t index = 0; index < groups.size(); index++) {
        const ConnectionGroup& group = groups[index];
        if (group.port->name == name) {
          capture.lexicalBinding(named.name, markGroup(group, true),
                                 LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL);
          if (!capture.declarationOnly)
            markByName(group);
          namedGroups[index] = true;
          connected = true;
          break;
        }
      }
      // Slang binds the actual of the first named connection of each port
      // only: one naming no port (PortDoesNotExist) or repeating a name
      // (DuplicatePortConnection) is reported and never bound.
      if (name.empty() || !namedSeen.insert(name).second)
        connected = false;
      if (!connected && named.expr)
        bindUnboundConnectionNames(symbol, *named.expr);
    }
    // `.*` connects every port not named explicitly by name (23.3.2.4).
    if (wildcard && !capture.declarationOnly) {
      for (size_t index = 0; index < groups.size(); index++)
        if (!namedGroups[index])
          markByName(groups[index]);
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

  // The actual of a named connection that Slang never binds (see
  // addConnectionLabelBindings) carries no expression, so none of its names
  // would get a token. Bind it here in the instance's connection context
  // (PortConnectionBuilder: the parent scope, after the instance), as Slang
  // binds a black-box actual (bindUnknownPortConn), and bind the names that
  // resolve lexically only: no semantic node is added, and no diagnostic
  // reaches the snapshot, which took them before capture. An assertion
  // actual, or one whose binding fails (an instance name is not a value), is
  // looked up name by name instead. Instance array elements share the
  // syntax, which is bound once.
  void bindUnboundConnectionNames(const InstanceSymbol& instance,
                                  const syntax::PropertyExprSyntax& actual) {
    const Scope* scope = instance.getParentScope();
    if (!scope || !unboundConnections.insert(&actual).second)
      return;
    const ASTContext context(*scope, LookupLocation::after(instance),
                             ASTFlags::NonProcedural);
    const syntax::ExpressionSyntax* expression = nullptr;
    if (actual.kind == syntax::SyntaxKind::SimplePropertyExpr) {
      const syntax::SequenceExprSyntax* sequence =
          actual.as<syntax::SimplePropertyExprSyntax>().expr;
      if (sequence->kind == syntax::SyntaxKind::SimpleSequenceExpr) {
        const auto& simple = sequence->as<syntax::SimpleSequenceExprSyntax>();
        if (!simple.repetition)
          expression = simple.expr;
      }
    }
    if (expression) {
      const Expression& bound = Expression::bind(
          *expression, context,
          ASTFlags::AllowUnboundedLiteral | ASTFlags::StreamingAllowed);
      bindInvalidSubtree(&bound);
      if (!bound.bad())
        return;
    }
    LookupNames names(*this, context);
    actual.visit(names);
  }

  // Connection actuals already bound by bindUnboundConnectionNames.
  std::unordered_set<const syntax::SyntaxNode*> unboundConnections;

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
      capture.semanticRole(id, &elementSelectBase(expression), LLG_SLANG_EDGE_BASE);
      capture.semanticRole(id, &expression.selector(), LLG_SLANG_EDGE_INDEX);
    }
    else if constexpr (std::same_as<T, RangeSelectExpression>) {
      capture.semanticRole(id, &rangeSelectBase(expression), LLG_SLANG_EDGE_BASE);
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LEFT);
      capture.semanticRole(id, &expression.right(), LLG_SLANG_EDGE_RIGHT);
    }
    else if constexpr (std::same_as<T, MemberAccessExpression>) {
      capture.semanticRole(id, &expression.value(), LLG_SLANG_EDGE_BASE);
      capture.semanticRole(id, &expression.member, LLG_SLANG_EDGE_REFERENCE);
    }
    else if constexpr (std::same_as<T, TaggedUnionExpression>) {
      if (expression.valueExpr)
        capture.semanticRole(id, expression.valueExpr, LLG_SLANG_EDGE_BODY);
    }
    else if constexpr (std::same_as<T, ConditionalExpression>) {
      for (uint32_t i = 0; i < expression.conditions.size(); i++) {
        capture.semanticRole(id, expression.conditions[i].expr,
                             LLG_SLANG_EDGE_CONDITION, i);
        // A pattern is not a Boolean expression. Retain its role even while
        // executable matching is unsupported, so import cannot erase it.
        if (expression.conditions[i].pattern)
          capture.semanticRole(id, expression.conditions[i].pattern,
                               LLG_SLANG_EDGE_CONDITION_PATTERN, i);
      }
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
    else if constexpr (std::same_as<T, SimpleAssignmentPatternExpression>) {
      // Synthesized default rows can repeat the same Expression pointer.
      // Structural children are deduplicated by attach(), but positional
      // operands must retain one edge per occurrence in elements().
      capture.removeChildEdges(id);
      uint32_t index = 0;
      for (const Expression* element : expression.elements())
        capture.semanticRole(id, element, LLG_SLANG_EDGE_OPERAND, index++);
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
    else if constexpr (std::same_as<T, NewCovergroupExpression>) {
      uint32_t index = 0;
      for (const Expression* argument : expression.arguments)
        capture.semanticRole(id, argument, LLG_SLANG_EDGE_ARGUMENT, index++);
    }
    else if constexpr (std::same_as<T, CopyClassExpression>) {
      capture.semanticRole(id, &expression.sourceExpr(), LLG_SLANG_EDGE_OPERAND);
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
    else if constexpr (std::same_as<T, DistExpression>) {
      capture.semanticRole(id, &expression.left(), LLG_SLANG_EDGE_LHS);
      uint32_t index = 0;
      for (const auto& item : expression.items()) {
        capture.semanticRole(id, &item.value, LLG_SLANG_EDGE_OPERAND, index);
        if (item.weight && item.weight->expr)
          capture.semanticRole(id, item.weight->expr,
                               item.weight->kind == DistExpression::DistWeight::PerRange
                                   ? LLG_SLANG_EDGE_RIGHT
                                   : LLG_SLANG_EDGE_INDEX,
                               index);
        index++;
      }
      if (const auto* weight = expression.defaultWeight()) {
        capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary |=
            LLG_SLANG_DIST_DEFAULT;
        if (weight->expr)
          capture.semanticRole(id, weight->expr, LLG_SLANG_EDGE_DEFAULT_VALUE);
      }
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
      if (expression.isSystemCall()) {
        const auto& info =
            std::get<CallExpression::SystemCallInfo>(expression.subroutine);
        if (info.subroutine->knownNameId == parsing::KnownSystemName::Randomize) {
          const bool classForm =
              info.scope->getCompilation().getSystemMethod(
                  SymbolKind::ClassType, expression.getSubroutineName()) ==
              info.subroutine.get();
          capture.output.semantic_nodes[static_cast<size_t>(id)].auxiliary |=
              classForm ? LLG_SLANG_CALL_CLASS_RANDOMIZE
                        : LLG_SLANG_CALL_SCOPE_RANDOMIZE;
        }
        if (const auto* randomize =
                std::get_if<CallExpression::RandomizeCallInfo>(&info.extraInfo);
            randomize && randomize->inlineConstraints &&
            randomize->inlineConstraints->kind != ConstraintKind::Invalid)
          capture.semanticRole(id, randomize->inlineConstraints, LLG_SLANG_EDGE_BODY);
      }
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

  bool useResolvedPatternElements(
      const StructuredAssignmentPatternExpression& expression) const {
    if (expression.bad())
      return false;

    const Type& type = expression.type->getCanonicalType();
    if (type.kind != SymbolKind::FixedSizeUnpackedArrayType)
      return false;

    // Slang binds an untyped nested default against an error type as a
    // placeholder, then binds executable values against each element. Keep
    // the resolved elements for that legacy path as well as type-keyed
    // patterns, including mixtures with explicit indices. The resolved list
    // preserves explicit-index precedence while recursively filling the other
    // elements; an immediate-type-only consumer cannot reconstruct that list.
    // Index-only patterns still retain their owned semantic key expressions.
    const bool bad_default = expression.defaultSetter &&
                             expression.defaultSetter->bad();
    const bool resolved_type_keys = !expression.typeSetters.empty();
    if (!bad_default && !resolved_type_keys)
      return false;

    const auto elements = expression.elements();
    const size_t expected =
        static_cast<size_t>(type.getFixedRange().fullWidth());

    if (elements.size() != expected)
      return false;
    for (const Expression* element : elements) {
      if (!element || element->bad())
        return false;
    }

    // Slang has already applied index, last matching type, and default
    // precedence, recursively descending through fixed arrays and records.
    return true;
  }

  void addStructuredPatternRoles(
      const StructuredAssignmentPatternExpression& expression, uint64_t id) {
    capture.removeChildEdges(id);
    uint32_t index = 0;
    if (useResolvedPatternElements(expression)) {
      auto elements = expression.elements();
      if (expression.type->getCanonicalType().kind ==
          SymbolKind::FixedSizeUnpackedArrayType) {
        // Slang stores fixed-array elements by increasing index. Positional
        // operands in the owned DB use declaration order.
        const auto range = expression.type->getFixedRange();
        for (size_t ordinal = 0; ordinal < elements.size(); ordinal++) {
          const size_t offset = range.isDescending()
              ? elements.size() - ordinal - 1 : ordinal;
          capture.semanticRole(id, elements[offset], LLG_SLANG_EDGE_OPERAND,
                               index++);
        }
      }
      else {
        // Structure elements are returned in declaration order.
        for (const Expression* element : elements)
          capture.semanticRole(id, element, LLG_SLANG_EDGE_OPERAND, index++);
      }
      return;
    }
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
      for (uint32_t i = 0; i < statement.conditions.size(); i++) {
        capture.semanticRole(id, statement.conditions[i].expr,
                             LLG_SLANG_EDGE_CONDITION, i);
        // A pattern is not a Boolean expression. Retain its role even while
        // executable matching is unsupported, so import cannot erase it.
        if (statement.conditions[i].pattern)
          capture.semanticRole(id, statement.conditions[i].pattern,
                               LLG_SLANG_EDGE_CONDITION_PATTERN, i);
      }
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
      if (const Expression* disable = defaultDisable(statement))
        capture.semanticRole(id, disable, LLG_SLANG_EDGE_DEFAULT_DISABLE);
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
    else if constexpr (std::same_as<T, PatternCaseStatement>) {
      capture.semanticRole(id, &statement.expr, LLG_SLANG_EDGE_CASE_EXPRESSION);
      uint32_t itemIndex = 0;
      for (const auto& item : statement.items) {
        // PatternCase items have one pattern and one body.  Reuse the
        // existing case-item/branch roles with the item index; the owned
        // importer distinguishes this statement kind and preserves the
        // optional filter separately.
        capture.semanticRole(id, item.pattern, LLG_SLANG_EDGE_CASE_ITEM, itemIndex);
        if (item.filter)
          capture.semanticRole(id, item.filter, LLG_SLANG_EDGE_CONDITION, itemIndex);
        capture.semanticRole(id, item.stmt, LLG_SLANG_EDGE_BRANCH, itemIndex);
        itemIndex++;
      }
      if (statement.defaultCase)
        capture.semanticRole(id, statement.defaultCase, LLG_SLANG_EDGE_ELSE);
    }
    else if constexpr (std::same_as<T, RandCaseStatement>) {
      uint32_t itemIndex = 0;
      for (const auto& item : statement.items) {
        capture.semanticRole(id, item.expr, LLG_SLANG_EDGE_CASE_ITEM, itemIndex);
        capture.semanticRole(id, item.stmt, LLG_SLANG_EDGE_BRANCH, itemIndex);
        itemIndex++;
      }
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
  // Typedef names have no dedicated semantic kind; Slang's symbol-kind name is
  // the exact discriminator, as for genvars above.
  if (detail == "TypeAlias")
    return LLG_SLANG_LEXICAL_TYPE_ALIAS;
  // Type and value parameters share the parameter kind.
  if (detail == "TypeParameter")
    return LLG_SLANG_LEXICAL_TYPE_PARAMETER;
  // Struct and union members and class properties share the variable kind.
  if (detail == "Field" || detail == "ClassProperty")
    return LLG_SLANG_LEXICAL_FIELD;
  // Sequences, properties and let declarations are invoked with arguments
  // like a function (IEEE 1800-2009 16.8, 16.13, 11.13) and have no
  // semantic kind of their own.
  if (detail == "Sequence" || detail == "Property" || detail == "LetDecl")
    return LLG_SLANG_LEXICAL_FUNCTION;
  // Named and labeled statement blocks share the scope kind with instance
  // bodies, compilation units and clocking blocks; the symbol kind tells them
  // apart.
  if (node.kind == LLG_SLANG_SEMANTIC_GENERATE_SCOPE ||
      (node.kind == LLG_SLANG_SEMANTIC_SCOPE && detail == "StatementBlock"))
    return LLG_SLANG_LEXICAL_BLOCK;
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

  // A definition that is not part of this compilation (an isolated buffer
  // that instantiates a module from another file) leaves the type name
  // unbound. Give it the module kind without a role so no navigation target is
  // implied; a resolved binding for the same token takes precedence.
  void handle(const syntax::HierarchyInstantiationSyntax& syntax) {
    capture.lexicalBinding(syntax.type, LLG_SLANG_INVALID_ID,
                           LLG_SLANG_LEXICAL_ROLE_NONE,
                           LLG_SLANG_LEXICAL_MODULE);
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

  // Block names come from the syntax so that every capture mode classifies
  // them, including declaration-only capture that builds no block scopes.
  // The opening name (a statement label or the `: name` after begin/fork) is
  // the declaration; the closing `: name` refers to it.
  void handle(const syntax::BlockStatementSyntax& syntax) {
    blockName(syntax.label ? &syntax.label->name : nullptr,
              syntax.blockName ? &syntax.blockName->name : nullptr,
              syntax.endBlockName ? &syntax.endBlockName->name : nullptr);
    visitDefault(syntax);
  }

  void handle(const syntax::GenerateBlockSyntax& syntax) {
    blockName(syntax.label ? &syntax.label->name : nullptr,
              syntax.beginName ? &syntax.beginName->name : nullptr,
              syntax.endName ? &syntax.endName->name : nullptr);
    visitDefault(syntax);
  }

  // A labeled statement that is not a block is still a named block scope
  // (Slang's StatementBlockSymbol::fromLabeledStmt; IEEE 1800-2009 9.3.5).
  void handle(const syntax::StatementSyntax& syntax) {
    if (syntax.label && syntax.kind != syntax::SyntaxKind::SequentialBlockStatement &&
        syntax.kind != syntax::SyntaxKind::ParallelBlockStatement)
      blockName(&syntax.label->name, nullptr, nullptr);
    // Words whose other uses are not control flow: `unique`, `unique0` and
    // `priority` qualifying an if or case (IEEE 1800-2009 A.6.6, A.6.7;
    // `unique` also starts a uniqueness constraint) and the `disable` of a
    // disable statement (A.6.5; `disable iff` is a property reset).
    switch (syntax.kind) {
      case syntax::SyntaxKind::ConditionalStatement:
        controlKeyword =
            syntax.as<syntax::ConditionalStatementSyntax>().uniqueOrPriority.location();
        break;
      case syntax::SyntaxKind::CaseStatement:
        controlKeyword =
            syntax.as<syntax::CaseStatementSyntax>().uniqueOrPriority.location();
        break;
      case syntax::SyntaxKind::DisableStatement:
        controlKeyword = syntax.as<syntax::DisableStatementSyntax>().disable.location();
        break;
      case syntax::SyntaxKind::DisableForkStatement:
        controlKeyword =
            syntax.as<syntax::DisableForkStatementSyntax>().disable.location();
        break;
      // Procedural continuous assignments (A.6.2; 10.6): `assign`,
      // `deassign`, `force` and `release` statements.
      case syntax::SyntaxKind::ProceduralAssignStatement:
      case syntax::SyntaxKind::ProceduralForceStatement:
        controlKeyword =
            syntax.as<syntax::ProceduralAssignStatementSyntax>().keyword.location();
        break;
      case syntax::SyntaxKind::ProceduralDeassignStatement:
      case syntax::SyntaxKind::ProceduralReleaseStatement:
        controlKeyword =
            syntax.as<syntax::ProceduralDeassignStatementSyntax>().keyword.location();
        break;
      default:
        break;
    }
    visitDefault(syntax);
  }

  // A `define directive. Its formal arguments (IEEE 1800-2009 22.5.1) are
  // the macro's parameters: the name in the formal list is their
  // declaration and every body token that spells one is a use. Every other
  // body name is ordinary source text once expanded; it takes the kind its
  // expansions bind at the body text (`Capture::macroBodySpan`).
  void handle(const syntax::DefineDirectiveSyntax& syntax) {
    defineName = syntax.name.location();
    defineFormals.clear();
    if (syntax.formalArguments) {
      for (const syntax::MacroFormalArgumentSyntax* formal :
           syntax.formalArguments->args)
        defineFormals.push_back(formal->name);
    }
    defineDepth++;
    visitDefault(syntax);
    defineDepth--;
    defineFormals.clear();
  }

  // The net type of `default_nettype (22.8) is a net type keyword, or
  // `none`, which is a keyword only here.
  void handle(const syntax::DefaultNetTypeDirectiveSyntax& syntax) {
    directiveKeyword = syntax.netType.location();
    visitDefault(syntax);
  }

  void handle(const syntax::StructUnionMemberSyntax& syntax) {
    for (const syntax::DeclaratorSyntax* declarator : syntax.declarators)
      capture.fieldDeclaration(declarator->name);
    visitDefault(syntax);
  }

  // Process introducers (IEEE 1800-2009 A.6.2, 9.2; also the checker
  // `always` of A.1.8): control flow, unlike the `initial` of a UDP initial
  // statement (A.5.4) and the `always` property operator (A.2.10).
  void handle(const syntax::ProceduralBlockSyntax& syntax) {
    controlKeyword = syntax.keyword.location();
    visitDefault(syntax);
  }

  // Continuous assignment and net alias (A.6.1; 10.3, 10.11).
  void handle(const syntax::ContinuousAssignSyntax& syntax) {
    controlKeyword = syntax.assign.location();
    visitDefault(syntax);
  }

  void handle(const syntax::NetAliasSyntax& syntax) {
    controlKeyword = syntax.keyword.location();
    visitDefault(syntax);
  }

  // `default` labels a case item (IEEE 1800-2009 12.5, 18.17.3, 16.13.16 and
  // the generate case of 27.5): control flow, unlike the `default` of an
  // assignment pattern key, clocking, disable iff, bins or dist item.
  void handle(const syntax::DefaultCaseItemSyntax& syntax) {
    controlKeyword = syntax.defaultKeyword.location();
    visitDefault(syntax);
  }

  void handle(const syntax::DefaultRsCaseItemSyntax& syntax) {
    controlKeyword = syntax.defaultKeyword.location();
    visitDefault(syntax);
  }

  void handle(const syntax::DefaultPropertyCaseItemSyntax& syntax) {
    controlKeyword = syntax.defaultKeyword.location();
    visitDefault(syntax);
  }

  void handle(const syntax::ConditionalBranchDirectiveSyntax& syntax) {
    visitToken(syntax.directive);
    syntax.expr->visit(*this);
    // Slang stores an untaken arm here instead of in skipped-token trivia.
    skippedDepth++;
    for (parsing::Token token : syntax.disabledTokens)
      visitToken(token);
    skippedDepth--;
  }

  void handle(const syntax::UnconditionalBranchDirectiveSyntax& syntax) {
    visitToken(syntax.directive);
    skippedDepth++;
    for (parsing::Token token : syntax.disabledTokens)
      visitToken(token);
    skippedDepth--;
  }

  // A macro usage at its use site. The name is the use-site token; the
  // written actual arguments follow it in the source. Each written argument
  // token is recorded so the expanded copy of the same text replaces it
  // (`addToken`): the expansion, not the raw argument list, is what the
  // parser and elaboration saw, so it carries the token's kind and binding.
  void handle(const syntax::MacroUsageSyntax& syntax) {
    visitToken(syntax.directive);
    if (!syntax.args)
      return;
    macroArgumentDepth++;
    syntax.args->visit(*this);
    macroArgumentDepth--;
  }

  // Records the logical position that a `line directive gives the following
  // physical line. The values are read back from the SourceManager, so they
  // match the preprocessor's `__LINE__`/`__FILE__` exactly, including for a
  // malformed directive that the preprocessor ignored. A directive on the last
  // line maps nothing and records nothing.
  void handle(const syntax::LineDirectiveSyntax& syntax) {
    lineDirective(syntax.directive.location());
    visitDefault(syntax);
  }

  // Protect pragma keywords (IEEE 1800-2009 34.5.1-34.5.4) that open and
  // close envelopes. Matching the keyword spelling is how the frontend's own
  // pragma handlers dispatch; one directive may carry several expressions.
  void handle(const syntax::PragmaDirectiveSyntax& syntax) {
    if (syntax.name.valueText() == "protect") {
      for (const syntax::PragmaExpressionSyntax* argument : syntax.args) {
        if (argument->kind != syntax::SyntaxKind::SimplePragmaExpression)
          continue;
        const std::string_view keyword =
            argument->as<syntax::SimplePragmaExpressionSyntax>().value.valueText();
        if (keyword == "begin_protected")
          protectOpen(syntax, LLG_SLANG_PROTECT_DECRYPTION, 0);
        else if (keyword == "end_protected")
          protectClose(syntax, LLG_SLANG_PROTECT_DECRYPTION, 0);
        else if (keyword == "begin")
          protectOpen(syntax, LLG_SLANG_PROTECT_ENCRYPTION, 0);
        else if (keyword == "end")
          protectClose(syntax, LLG_SLANG_PROTECT_ENCRYPTION, 0);
      }
    }
    visitDefault(syntax);
  }

  // Verilog-XL envelope directives (lexed with legacy protect enabled).
  void handle(const syntax::SimpleDirectiveSyntax& syntax) {
    switch (syntax.kind) {
      case syntax::SyntaxKind::ProtectedDirective:
        protectOpen(syntax, LLG_SLANG_PROTECT_DECRYPTION, LLG_SLANG_PROTECT_LEGACY);
        break;
      case syntax::SyntaxKind::EndProtectedDirective:
        protectClose(syntax, LLG_SLANG_PROTECT_DECRYPTION, LLG_SLANG_PROTECT_LEGACY);
        break;
      case syntax::SyntaxKind::ProtectDirective:
        protectOpen(syntax, LLG_SLANG_PROTECT_ENCRYPTION, LLG_SLANG_PROTECT_LEGACY);
        break;
      case syntax::SyntaxKind::EndProtectDirective:
        protectClose(syntax, LLG_SLANG_PROTECT_ENCRYPTION, LLG_SLANG_PROTECT_LEGACY);
        break;
      default:
        break;
    }
    visitDefault(syntax);
  }

  // Emits the envelopes still open at the end of the walk (no closing
  // directive in their file) and orders all records by file and start. An
  // include file parsed into several trees reports each envelope once.
  void finishProtectedEnvelopes() {
    for (auto& [file, open] : openEnvelopes) {
      const uint64_t fileLen = capture.output.files[static_cast<size_t>(file)].byte_len;
      for (const LlgSlangProtectedEnvelope& envelope : open) {
        LlgSlangProtectedEnvelope record = envelope;
        record.range.end = fileLen;
        record.flags |= LLG_SLANG_PROTECT_UNTERMINATED;
        pushProtectedEnvelope(record);
      }
    }
    openEnvelopes.clear();
    auto& envelopes = capture.output.protected_envelopes;
    std::sort(envelopes.begin(), envelopes.end(), [](const auto& left, const auto& right) {
      return std::tie(left.range.file_id, left.range.start, left.range.end) <
             std::tie(right.range.file_id, right.range.start, right.range.end);
    });
    envelopes.erase(std::unique(envelopes.begin(), envelopes.end(),
                                [](const auto& left, const auto& right) {
                                  return left.range.file_id == right.range.file_id &&
                                         left.range.start == right.range.start;
                                }),
                    envelopes.end());
  }

  // Orders the FSM pragma records (an include file parsed into several trees
  // reports each comment once) and appends one instance record per
  // elaborated instance of every declaration that holds a pragma. Designs
  // without pragmas pay nothing here.
  void finishFsmPragmas(const RootSymbol& root) {
    auto& records = capture.output.fsm_pragmas;
    std::sort(records.begin(), records.end(), [](const auto& left, const auto& right) {
      return std::tie(left.range.file_id, left.range.start, left.ordinal) <
             std::tie(right.range.file_id, right.range.start, right.ordinal);
    });
    records.erase(std::unique(records.begin(), records.end(),
                              [](const auto& left, const auto& right) {
                                return left.range.file_id == right.range.file_id &&
                                       left.range.start == right.range.start &&
                                       left.ordinal == right.ordinal;
                              }),
                  records.end());
    if (fsmPragmaScopes.empty())
      return;
    struct Instances : public ASTVisitor<Instances, VisitFlags::Symbols> {
      Instances(LexicalCapture& owner) : owner(owner) {}
      void handle(const InstanceSymbol& instance) {
        const syntax::SyntaxNode* syntax = instance.getDefinition().getSyntax();
        if (auto scope = owner.fsmPragmaScopes.find(syntax);
            syntax && scope != owner.fsmPragmaScopes.end()) {
          if (auto id = owner.capture.semanticIds.find(&instance);
              id != owner.capture.semanticIds.end()) {
            chargeRecord(owner.capture.output, sizeof(LlgSlangFsmPragma));
            owner.capture.output.fsm_pragmas.push_back(
                {{LLG_SLANG_INVALID_ID, 0, 0}, scope->second, {LLG_SLANG_INVALID_ID, 0, 0},
                 id->second, LLG_SLANG_FSM_INSTANCE, 0, 0, 0, {nullptr, 0}});
          }
        }
        visitDefault(instance);
      }
      LexicalCapture& owner;
    };
    Instances instances(*this);
    root.visit(instances);
  }

  // Declarations that can hold protected text (IEEE 1800-2009 34.3: an
  // envelope may replace any part of the source). Their names identify the
  // enclosing definition of an envelope.
  void handle(const syntax::ModuleDeclarationSyntax& syntax) {
    designScopes.push_back({syntax.getFirstToken().location(), syntax.header->name});
    // FSM pragmas belong to module, interface and program definitions
    // (IEEE 1800-2009 40.4.1), never to packages.
    const bool fsmScope = syntax.kind != syntax::SyntaxKind::PackageDeclaration;
    if (fsmScope)
      fsmScopes.push_back(&syntax);
    visitDefault(syntax);
    if (fsmScope)
      fsmScopes.pop_back();
    designScopes.pop_back();
    // The frontend fabricates a missing end keyword at the end of the file;
    // protected text directly after the declaration holds it.
    if (syntax.endmodule.isMissing() && !missingEndScope)
      missingEndScope = syntax.header->name;
  }

  // Declarations whose first declarator can carry an FSM `tool enum` pragma
  // (IEEE 1800-2009 40.4.1, 40.4.4-40.4.6): the comment follows the type and
  // its packed dimensions, so it is trivia of the first declarator name.
  void handle(const syntax::DataDeclarationSyntax& syntax) {
    fsmDeclaration(syntax, syntax.declarators, LLG_SLANG_FSM_SIGNAL_DECLARATOR);
  }

  void handle(const syntax::NetDeclarationSyntax& syntax) {
    fsmDeclaration(syntax, syntax.declarators, LLG_SLANG_FSM_SIGNAL_DECLARATOR);
  }

  void handle(const syntax::PortDeclarationSyntax& syntax) {
    fsmDeclaration(syntax, syntax.declarators, LLG_SLANG_FSM_SIGNAL_DECLARATOR);
  }

  void handle(const syntax::ParameterDeclarationSyntax& syntax) {
    fsmDeclaration(syntax, syntax.declarators, LLG_SLANG_FSM_PARAMETER_DECLARATOR);
  }

  void handle(const syntax::ImplicitAnsiPortSyntax& syntax) {
    const FsmDeclarators previous = fsmDeclarators;
    fsmDeclarators = {};
    if (!syntax.declarator->name.isMissing())
      fsmDeclarators = {syntax.declarator->name.location(), LLG_SLANG_FSM_SIGNAL_DECLARATOR,
                        nullptr, syntax.declarator->name};
    visitDefault(syntax);
    fsmDeclarators = previous;
  }

  void visitToken(parsing::Token token) {
    if (directiveDepth == 0) {
      // Trivia before a declaration's first token lies outside it.
      size_t depth = designScopes.size();
      if (depth != 0 && token.location() == designScopes.back().first)
        depth--;
      triviaScope = depth != 0 ? std::optional(designScopes[depth - 1].second)
                               : std::nullopt;
      if (skippedDepth == 0)
        fsmComments(token);
    }
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
    if (directiveDepth == 0 && !token.isMissing() &&
        token.kind != parsing::TokenKind::EndOfFile)
      missingEndScope.reset();
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
    const auto rangeLess = [](const LlgSlangSourceRange& left,
                              const LlgSlangSourceRange& right) {
      return std::tie(left.file_id, left.start, left.end) <
             std::tie(right.file_id, right.start, right.end);
    };
    std::sort(capture.fieldDeclarations.begin(), capture.fieldDeclarations.end(),
              rangeLess);
    for (const auto& binding : capture.lexicalBindings) {
      if (binding.range.file_id == LLG_SLANG_INVALID_ID)
        continue;
      const auto key = std::tuple{binding.range.file_id, binding.range.start,
                                  binding.range.end};
      auto [it, inserted] = connectionLabels.try_emplace(key, binding);
      if (inserted)
        continue;
      if (binding.macroBody || it->second.macroBody) {
        mergeMacroBodyBinding(it->second, binding);
        continue;
      }
      // An implicit named port connection `.name` (IEEE 1800-2009 23.3.2.3)
      // is both the label and a reference to the connected name; the label
      // role wins so the result does not depend on capture order.
      const bool existingLabel =
          it->second.role == LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL;
      const bool newLabel = binding.role == LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL;
      if ((newLabel && !existingLabel) ||
          (newLabel == existingLabel &&
           it->second.semantic_id == LLG_SLANG_INVALID_ID &&
           binding.semantic_id != LLG_SLANG_INVALID_ID)) {
        it->second = binding;
      }
    }
    for (size_t nodeIndex = 0; nodeIndex < capture.output.semantic_nodes.size();
         nodeIndex++) {
      const auto& node = capture.output.semantic_nodes[nodeIndex];
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
      // A transparent member is Slang's alias that makes an enum value
      // visible in the enclosing scope; it shares the value's name and
      // location but is not a declaration, and would otherwise hide the
      // value's node.
      const bool transparentMember = std::string_view(
          reinterpret_cast<const char*>(node.detail.data), node.detail.len) ==
          "TransparentMember";
      if (!sourceGenvarClone && !transparentMember &&
          node.kind != LLG_SLANG_SEMANTIC_EXPRESSION && node.name.len != 0) {
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
        // A scope-qualified enum value (`pkg::ITEM`) spans more than the
        // value's name, so its first token is the qualifier, not the value.
        const bool qualifiedEnumValue =
            lexicalTarget < capture.output.semantic_nodes.size() &&
            capture.output.semantic_nodes[static_cast<size_t>(lexicalTarget)]
                    .kind == LLG_SLANG_SEMANTIC_ENUM_CONSTANT &&
            node.range.end - node.range.start !=
                capture.output.semantic_nodes[static_cast<size_t>(lexicalTarget)]
                    .name.len;
        if (node.subkind == LLG_SLANG_EXPR_REFERENCE &&
            detail != "HierarchicalValue" && !qualifiedEnumValue) {
          referencesByStart.try_emplace(
              Position{node.range.file_id, node.range.start}, lexicalTarget);
        }
        referencesByEnd.try_emplace(Position{node.range.file_id, node.range.end},
                                    lexicalTarget);
      }
    }

    for (auto& token : capture.output.lexical_tokens) {
      const bool expandedMacro = token.kind == LLG_SLANG_LEXICAL_MACRO &&
          (token.flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION) != 0;
      // The array methods `unique`, `and`, `or` and `xor` (IEEE 1800-2009
      // 7.12.1, 7.12.3) are spelled as keywords; the call binding makes them
      // built-in methods instead.
      if (token.kind == LLG_SLANG_LEXICAL_KEYWORD) {
        auto it = connectionLabels.find(
            {token.range.file_id, token.range.start, token.range.end});
        if (it != connectionLabels.end() &&
            it->second.kind == LLG_SLANG_LEXICAL_SYSTEM_SUBROUTINE) {
          token.kind = LLG_SLANG_LEXICAL_SYSTEM_SUBROUTINE;
          token.role = LLG_SLANG_LEXICAL_ROLE_NONE;
        }
        continue;
      }
      if ((token.kind != LLG_SLANG_LEXICAL_IDENTIFIER && !expandedMacro) ||
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
        // An enum range `R[3]` or `R[1:3]` declares R0..R2 / R1..R3 (IEEE
        // 1800-2009 6.19.2) at the name token R: the generated names are the
        // token text followed by decimal digits.
        const bool enumRangeName =
            declaration.kind == LLG_SLANG_SEMANTIC_ENUM_CONSTANT &&
            declarationName.size() > tokenText.size() &&
            declarationName.starts_with(tokenText) &&
            std::all_of(declarationName.begin() + tokenText.size(),
                        declarationName.end(),
                        [](char c) { return c >= '0' && c <= '9'; });
        if (tokenText == declarationName || enumRangeName) {
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
      if (role == LLG_SLANG_LEXICAL_ROLE_NONE &&
          std::binary_search(capture.fieldDeclarations.begin(),
                             capture.fieldDeclarations.end(), token.range,
                             rangeLess)) {
        role = LLG_SLANG_LEXICAL_ROLE_DECLARATION;
        boundKind = LLG_SLANG_LEXICAL_FIELD;
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
      if (role == LLG_SLANG_LEXICAL_ROLE_REFERENCE ||
          role == LLG_SLANG_LEXICAL_ROLE_CONNECTION_ACTUAL) {
        auto declaration = capture.unitValueDeclarations.find(semanticId);
        auto location = tokenLocations.find(
            std::tuple{token.range.file_id, token.range.start, token.range.end});
        if (declaration != capture.unitValueDeclarations.end() &&
            location != tokenLocations.end() &&
            capture.sourceManager.isBeforeInCompilationUnit(
                location->second, declaration->second).value_or(false))
          token.flags |= LLG_SLANG_LEXICAL_UNIT_FORWARD_REFERENCE;
      }
      if (expandedMacro) continue;
      if (role != LLG_SLANG_LEXICAL_ROLE_NONE) {
        token.role = role;
        if (boundKind != LLG_SLANG_LEXICAL_UNKNOWN)
          token.kind = boundKind;
      }
      else if (boundKind != LLG_SLANG_LEXICAL_UNKNOWN) {
        token.kind = boundKind;
      }
      if (semanticId != LLG_SLANG_INVALID_ID &&
          semanticId < capture.output.semantic_nodes.size()) {
        const auto& semantic = capture.output.semantic_nodes[semanticId];
        token.semantic_id = semanticId;
        token.kind = lexicalKindForSemantic(semantic);
      }
    }
    bindBlockNames();
  }

private:
  // The lexical kind a binding gives its token.
  uint32_t boundLexicalKind(const Capture::LexicalBinding& binding) const {
    const uint64_t target = capture.canonicalLexicalTarget(binding.semantic_id);
    if (target != LLG_SLANG_INVALID_ID && target < capture.output.semantic_nodes.size())
      return lexicalKindForSemantic(
          capture.output.semantic_nodes[static_cast<size_t>(target)]);
    return binding.kind;
  }

  // Every expansion of a macro binds the names of its `define body again.
  // The body token keeps a kind and role only when every binding agrees on
  // them, and a target only when they also agree on the target; bindings
  // that disagree (`v` a variable in one expansion and a type in another)
  // leave an unbound identifier, which gets no token.
  void mergeMacroBodyBinding(Capture::LexicalBinding& merged,
                             const Capture::LexicalBinding& binding) const {
    if (merged.kind == LLG_SLANG_LEXICAL_IDENTIFIER &&
        merged.role == LLG_SLANG_LEXICAL_ROLE_NONE)
      return;
    const uint32_t kind = boundLexicalKind(merged);
    if (!binding.macroBody || !merged.macroBody || kind != boundLexicalKind(binding) ||
        merged.role != binding.role) {
      merged = {merged.range, LLG_SLANG_INVALID_ID, LLG_SLANG_LEXICAL_ROLE_NONE,
                LLG_SLANG_LEXICAL_IDENTIFIER, true};
      return;
    }
    if (capture.canonicalLexicalTarget(merged.semantic_id) !=
        capture.canonicalLexicalTarget(binding.semantic_id)) {
      merged.kind = kind;
      merged.semantic_id = LLG_SLANG_INVALID_ID;
    }
  }

  // One block name declaration and the names that repeat it (an end label, or
  // a begin name after a statement label). Ranges are physical name spans.
  struct BlockName {
    LlgSlangSourceRange declaration;
    LlgSlangSourceRange repeats[2];
  };

  void blockName(const parsing::Token* label, const parsing::Token* begin,
                 const parsing::Token* end) {
    const parsing::Token* declaration = label ? label : begin;
    if (!declaration || declaration->isMissing())
      return;
    const auto range = capture.tokenSpan(*declaration);
    if (range.file_id == LLG_SLANG_INVALID_ID)
      return;
    if (blockNames.size() >= capture.maxLexicalTokens())
      throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                          "block name limit exceeded");
    BlockName name{range,
                   {{LLG_SLANG_INVALID_ID, 0, 0}, {LLG_SLANG_INVALID_ID, 0, 0}}};
    size_t repeat = 0;
    for (const parsing::Token* token : {label ? begin : nullptr, end}) {
      // A mismatched end label is a frontend error and names nothing.
      if (token && !token->isMissing() &&
          token->valueText() == declaration->valueText())
        name.repeats[repeat++] = capture.tokenSpan(*token);
    }
    blockNames.push_back(name);
  }

  LlgSlangLexicalToken* tokenAt(const LlgSlangSourceRange& range) {
    auto& tokens = capture.output.lexical_tokens;
    const auto it = std::lower_bound(
        tokens.begin(), tokens.end(), range, [](const auto& token, const auto& key) {
          return std::tie(token.range.file_id, token.range.start, token.range.end) <
                 std::tie(key.file_id, key.start, key.end);
        });
    if (it == tokens.end() || it->range.file_id != range.file_id ||
        it->range.start != range.start || it->range.end != range.end)
      return nullptr;
    return &*it;
  }

  // Runs after semantic binding over the sorted token table. A declaration
  // that capture bound to its block scope already has the block kind; one
  // without a scope (declaration-only capture, or a block Slang did not
  // build) still gets the kind and the declaration role from its syntax. A
  // repeated name refers to the declaration's block, or carries only the kind
  // when the declaration has no target. A name bound to anything else keeps
  // its binding.
  void bindBlockNames() {
    for (const auto& name : blockNames) {
      LlgSlangLexicalToken* declaration = tokenAt(name.declaration);
      if (!declaration || (declaration->flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION))
        continue;
      if (declaration->kind == LLG_SLANG_LEXICAL_IDENTIFIER &&
          declaration->semantic_id == LLG_SLANG_INVALID_ID) {
        declaration->kind = LLG_SLANG_LEXICAL_BLOCK;
        declaration->role = LLG_SLANG_LEXICAL_ROLE_DECLARATION;
      }
      if (declaration->kind != LLG_SLANG_LEXICAL_BLOCK)
        continue;
      for (const auto& range : name.repeats) {
        if (range.file_id == LLG_SLANG_INVALID_ID)
          continue;
        LlgSlangLexicalToken* repeat = tokenAt(range);
        if (!repeat || repeat->kind != LLG_SLANG_LEXICAL_IDENTIFIER ||
            repeat->semantic_id != LLG_SLANG_INVALID_ID ||
            (repeat->flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION))
          continue;
        repeat->kind = LLG_SLANG_LEXICAL_BLOCK;
        if (declaration->semantic_id != LLG_SLANG_INVALID_ID) {
          repeat->semantic_id = declaration->semantic_id;
          repeat->role = LLG_SLANG_LEXICAL_ROLE_REFERENCE;
        }
      }
    }
  }

  void protectOpen(const syntax::SyntaxNode& directive, uint32_t kind, uint32_t flags) {
    const LlgSlangSourceRange range = capture.span(directive.sourceRange());
    if (range.file_id == LLG_SLANG_INVALID_ID)
      return;
    LlgSlangSourceRange scope{LLG_SLANG_INVALID_ID, 0, 0};
    if (triviaScope)
      scope = capture.span(triviaScope->range());
    else if (missingEndScope)
      scope = capture.span(missingEndScope->range());
    // The end of one syntax tree is followed by the next tree's tokens; a
    // declaration only owns envelopes of its own file.
    if (scope.file_id != range.file_id)
      scope = {LLG_SLANG_INVALID_ID, 0, 0};
    openEnvelopes[range.file_id].push_back({range, scope, kind, flags});
  }

  // Closes the innermost open envelope of the same kind and form in the
  // directive's file; an unmatched closing directive (which the frontend
  // reports) closes nothing.
  void protectClose(const syntax::SyntaxNode& directive, uint32_t kind, uint32_t flags) {
    const LlgSlangSourceRange range = capture.span(directive.sourceRange());
    if (range.file_id == LLG_SLANG_INVALID_ID)
      return;
    auto found = openEnvelopes.find(range.file_id);
    if (found == openEnvelopes.end() || found->second.empty())
      return;
    LlgSlangProtectedEnvelope envelope = found->second.back();
    if (envelope.kind != kind || envelope.flags != flags)
      return;
    found->second.pop_back();
    envelope.range.end = std::max(envelope.range.start, range.end);
    pushProtectedEnvelope(envelope);
  }

  void pushProtectedEnvelope(const LlgSlangProtectedEnvelope& envelope) {
    chargeRecord(capture.output, sizeof(LlgSlangProtectedEnvelope));
    capture.output.protected_envelopes.push_back(envelope);
  }

  // The FSM pragma comments in the leading trivia of `token` (IEEE 1800-2009
  // 40.4.7: block and one-line comments). Trivia carry no position of their
  // own; walking back from the token, each one ends where the next starts,
  // and a directive gives its own start. Tokens copied from a macro body
  // have no source trivia.
  void fsmComments(parsing::Token token) {
    const auto trivia = token.trivia();
    bool comment = false;
    for (const parsing::Trivia& item : trivia) {
      comment |= item.kind == parsing::TriviaKind::LineComment ||
                 item.kind == parsing::TriviaKind::BlockComment;
      // A comment before a directive is trivia of the directive's first
      // token.
      if (const syntax::SyntaxNode* directive = item.syntax();
          directive && item.kind == parsing::TriviaKind::Directive)
        fsmComments(directive->getFirstToken());
    }
    SourceLocation cursor = token.location();
    if (!comment || !cursor.valid() || !capture.sourceManager.isFileLoc(cursor))
      return;
    // Trivia before a declaration's first token lies outside it.
    size_t depth = fsmScopes.size();
    if (depth != 0 && cursor == fsmScopes.back()->getFirstToken().location())
      depth--;
    const syntax::ModuleDeclarationSyntax* scope =
        depth != 0 ? fsmScopes[depth - 1] : nullptr;
    const bool declarator = fsmDeclarators.flag != 0 && cursor == fsmDeclarators.first;
    for (size_t i = trivia.size(); i-- > 0;) {
      const parsing::Trivia& item = trivia[i];
      if (auto location = item.getExplicitLocation()) {
        cursor = *location;
      }
      else {
        const size_t length = item.getRawText().size();
        if (cursor.offset() < length)
          return;
        cursor = cursor - length;
      }
      if (item.kind == parsing::TriviaKind::LineComment ||
          item.kind == parsing::TriviaKind::BlockComment)
        fsmComment(item, cursor, scope, declarator);
    }
  }

  void fsmComment(const parsing::Trivia& item, SourceLocation start,
                  const syntax::ModuleDeclarationSyntax* scopeSyntax, bool declarator) {
    const std::string_view raw = item.getRawText();
    const bool line = item.kind == parsing::TriviaKind::LineComment;
    std::string_view text;
    if (line) {
      if (!raw.starts_with("//"))
        return;
      text = raw.substr(2);
    }
    else {
      // An unterminated block comment is a frontend error, not a pragma.
      if (raw.size() < 4 || !raw.starts_with("/*") || !raw.ends_with("*/"))
        return;
      text = raw.substr(2, raw.size() - 4);
    }
    // The pragma keyword `tool` comes first (40.4.1); any other comment is
    // ordinary text.
    const auto blank = [](char c) {
      return c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '\f' || c == '\v';
    };
    size_t at = 0;
    while (at < text.size() && blank(text[at]))
      at++;
    if (text.substr(at, 4) != "tool" || at + 4 >= text.size() || !blank(text[at + 4]))
      return;
    const LlgSlangSourceRange range = capture.span(start, raw.size());
    if (range.file_id == LLG_SLANG_INVALID_ID)
      return;
    LlgSlangSourceRange scope{LLG_SLANG_INVALID_ID, 0, 0};
    if (scopeSyntax) {
      const parsing::Token name = scopeSyntax->header->name;
      scope = capture.span(name.location(), name.valueText().size());
      fsmPragmaScopes.emplace(scopeSyntax, scope);
    }
    const uint32_t flags = line ? LLG_SLANG_FSM_LINE_COMMENT : 0u;
    const LlgSlangString stored = storeString(capture.output, text);
    const auto push = [&](uint32_t flag, LlgSlangSourceRange name, uint32_t ordinal) {
      chargeRecord(capture.output, sizeof(LlgSlangFsmPragma));
      capture.output.fsm_pragmas.push_back({range, scope, name, LLG_SLANG_INVALID_ID,
                                            LLG_SLANG_FSM_PRAGMA, flags | flag, ordinal, 0,
                                            stored});
    };
    if (!declarator) {
      push(0, {LLG_SLANG_INVALID_ID, 0, 0}, 0);
      return;
    }
    const auto bind = [&](parsing::Token name, uint32_t ordinal) {
      // The declarator's semantic nodes span its name's value text.
      push(fsmDeclarators.flag, capture.span(name.location(), name.valueText().size()),
           ordinal);
    };
    if (fsmDeclarators.list) {
      uint32_t ordinal = 0;
      for (const syntax::DeclaratorSyntax* item : *fsmDeclarators.list)
        bind(item->name, ordinal++);
    }
    else {
      bind(fsmDeclarators.single, 0);
    }
  }

  template<typename Syntax>
  void fsmDeclaration(const Syntax& syntax,
                      const syntax::SeparatedSyntaxList<syntax::DeclaratorSyntax>& list,
                      uint32_t flag) {
    const FsmDeclarators previous = fsmDeclarators;
    fsmDeclarators = {};
    if (!list.empty() && !list[0]->name.isMissing())
      fsmDeclarators = {list[0]->name.location(), flag, &list, {}};
    visitDefault(syntax);
    fsmDeclarators = previous;
  }

  void lineDirective(SourceLocation location) {
    const SourceLocation physical = capture.physicalLocation(location);
    if (!physical.valid())
      return;
    const uint64_t file = capture.findFile(physical.buffer());
    if (file == LLG_SLANG_INVALID_ID)
      return;
    const std::string_view text =
        capture.sourceManager.getSourceText(physical.buffer());
    size_t next = text.find_first_of("\r\n", physical.offset());
    if (next == std::string_view::npos)
      return;
    next += (text[next] == '\r' && next + 1 < text.size() && text[next + 1] == '\n')
                ? 2
                : 1;
    // Slang buffers end in a NUL sentinel; a line must start before it.
    if (next >= text.size() || text[next] == '\0')
      return;
    const SourceLocation mapped(physical.buffer(), next);
    chargeRecord(capture.output, sizeof(LlgSlangLineDirective));
    capture.output.line_directives.push_back(
        {file, static_cast<uint64_t>(next),
         static_cast<uint64_t>(capture.sourceManager.getLineNumber(mapped)),
         storeString(capture.output, capture.sourceManager.getFileName(mapped))});
  }

  Capture& capture;
  std::unordered_set<const syntax::SyntaxNode*> directives;
  // Open protected envelopes per file, innermost last.
  std::map<uint64_t, std::vector<LlgSlangProtectedEnvelope>> openEnvelopes;
  // Enclosing module/interface/program/package declarations: first token and
  // declared name, innermost last.
  std::vector<std::pair<SourceLocation, parsing::Token>> designScopes;
  // The declaration enclosing the trivia of the token being visited.
  std::optional<parsing::Token> triviaScope;
  // The innermost declaration whose end keyword is missing, until the next
  // real token.
  std::optional<parsing::Token> missingEndScope;
  // Enclosing module, interface and program declarations, innermost last.
  std::vector<const syntax::ModuleDeclarationSyntax*> fsmScopes;
  // The declaration being visited whose first declarator name can carry an
  // FSM `tool enum` pragma: that name's location and the declarators.
  struct FsmDeclarators {
    SourceLocation first;
    uint32_t flag = 0;
    const syntax::SeparatedSyntaxList<syntax::DeclaratorSyntax>* list = nullptr;
    parsing::Token single;
  };
  FsmDeclarators fsmDeclarators;
  // The declarations that hold FSM pragmas, with their scope ranges.
  std::unordered_map<const syntax::SyntaxNode*, LlgSlangSourceRange> fsmPragmaScopes;
  uint32_t directiveDepth = 0;
  uint32_t skippedDepth = 0;
  // Nesting depth inside the written actual arguments of a macro usage.
  uint32_t macroArgumentDepth = 0;
  std::map<std::tuple<uint64_t, uint64_t, uint64_t>, SourceLocation> tokenLocations;
  // Written macro argument tokens by physical range, as indices into the
  // lexical token table; an expanded argument token replaces its record.
  std::map<std::tuple<uint64_t, uint64_t, uint64_t>, size_t> writtenArguments;
  std::vector<BlockName> blockNames;
  // The control-flow keyword of the syntax node being visited.
  SourceLocation controlKeyword;
  // The macro name of the `define directive being visited.
  SourceLocation defineName;
  // The formal argument names of the `define directive being visited, and
  // whether one is being visited.
  std::vector<parsing::Token> defineFormals;
  uint32_t defineDepth = 0;
  // A directive operand that is a keyword although Slang lexes it as an
  // identifier (the `none` of `default_nettype none).
  SourceLocation directiveKeyword;

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
    if (skippedDepth != 0)
      flags |= LLG_SLANG_LEXICAL_SKIPPED;
    if (token.isMissing())
      flags |= LLG_SLANG_LEXICAL_MISSING;
    LlgSlangSourceRange range{LLG_SLANG_INVALID_ID, 0, 0};
    if (token.location().valid() &&
        capture.sourceManager.isMacroLoc(token.location())) {
      flags |= LLG_SLANG_LEXICAL_MACRO_EXPANSION;
      if (auto written = capture.macroArgumentSpan(token.location(), text)) {
        flags |= LLG_SLANG_LEXICAL_MACRO_ARGUMENT;
        range = *written;
      }
    }
    if ((flags & LLG_SLANG_LEXICAL_MACRO_ARGUMENT) == 0)
      range = capture.span(token.range());
    if (kind == LLG_SLANG_LEXICAL_KEYWORD && controlKeyword.valid() &&
        token.location() == controlKeyword)
      flags |= LLG_SLANG_LEXICAL_CONTROL_FLOW;
    // A `$name` is a system task or function by its spelling (IEEE 1800-2009
    // 5.6.3: "A name following the $ is interpreted as a system task or a
    // system function"), so it keeps the system kind wherever it is written:
    // constant contexts the semantic capture does not visit (dimensions,
    // generate conditions), a name Slang reports as unknown, recovered
    // syntax and macro bodies. Only an expansion copy that no written
    // argument maps back to stays macro text, so nothing of a body is
    // painted over a use site.
    const bool expansionCopy =
        (flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION) != 0 &&
        (flags & LLG_SLANG_LEXICAL_MACRO_ARGUMENT) == 0 && macroArgumentDepth == 0;
    if (token.kind == parsing::TokenKind::SystemIdentifier && !expansionCopy)
      kind = LLG_SLANG_LEXICAL_SYSTEM_SUBROUTINE;
    // Names in directives and expansion copies are macro text. Names written
    // in macro arguments and their expanded copies are ordinary names, and
    // so are the names of a `define body (bound from its expansions); the
    // formal arguments of a `define are its parameters.
    uint32_t role = kind == LLG_SLANG_LEXICAL_KEYWORD ? LLG_SLANG_LEXICAL_ROLE_KEYWORD
                                                      : LLG_SLANG_LEXICAL_ROLE_NONE;
    if (kind == LLG_SLANG_LEXICAL_IDENTIFIER && directiveDepth != 0 &&
        token.location() == directiveKeyword) {
      kind = LLG_SLANG_LEXICAL_KEYWORD;
      role = LLG_SLANG_LEXICAL_ROLE_KEYWORD;
    }
    else if (kind == LLG_SLANG_LEXICAL_IDENTIFIER && macroArgumentDepth == 0 &&
             (flags & LLG_SLANG_LEXICAL_MACRO_ARGUMENT) == 0 &&
             (directiveDepth != 0 ||
              (flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION) != 0)) {
      kind = LLG_SLANG_LEXICAL_MACRO;
      if (defineName.valid() && token.location() == defineName) {
        role = LLG_SLANG_LEXICAL_ROLE_DECLARATION;
      }
      else if (defineDepth != 0 && directiveDepth != 0 &&
               (flags & LLG_SLANG_LEXICAL_MACRO_EXPANSION) == 0) {
        kind = LLG_SLANG_LEXICAL_IDENTIFIER;
        for (const parsing::Token& formal : defineFormals) {
          if (formal.valueText() == token.valueText()) {
            kind = LLG_SLANG_LEXICAL_MACRO_FORMAL;
            if (formal.location() == token.location())
              role = LLG_SLANG_LEXICAL_ROLE_DECLARATION;
            break;
          }
        }
      }
    }
    const auto key = std::tuple{range.file_id, range.start, range.end};
    tokenLocations.try_emplace(key, token.location());
    const LlgSlangLexicalToken record{range, kind, role, flags, 0,
                                      LLG_SLANG_INVALID_ID, {}};
    if ((flags & LLG_SLANG_LEXICAL_MACRO_ARGUMENT) != 0 &&
        range.file_id != LLG_SLANG_INVALID_ID) {
      // Replace the written copy in place; repeated uses of one argument in
      // a body share its single record. The text is the same spelling.
      if (auto it = writtenArguments.find(key); it != writtenArguments.end()) {
        auto& written = capture.output.lexical_tokens[it->second];
        const LlgSlangString stored = written.text;
        written = record;
        written.text = stored;
        return;
      }
    }
    chargeRecord(capture.output, sizeof(LlgSlangLexicalToken));
    capture.output.lexical_tokens.push_back(record);
    capture.output.lexical_tokens.back().text = storeString(capture.output, text);
    // An expanded argument without a written copy (none was visited) is
    // registered too, so a second expansion of it still keeps one record.
    if ((macroArgumentDepth != 0 || (flags & LLG_SLANG_LEXICAL_MACRO_ARGUMENT) != 0) &&
        range.file_id != LLG_SLANG_INVALID_ID)
      writtenArguments.try_emplace(key, capture.output.lexical_tokens.size() - 1);
  }
};

// Strict IEEE 1364-2001 grammar profile over the parsed syntax trees.
//
// The frontend parses 2001 sources with its 1364-2005/SystemVerilog grammar
// and a 1364-2001 keyword table, so a later form that needs no new keyword
// still parses. The syntax trees are built from the macro-expanded token
// stream, so checking them sees a form built by a macro exactly like a
// spelled one. The profile is closed: every syntax kind not listed by
// verilog2001SyntaxKind() is a later form, and listed kinds get shape checks
// for the optional parts that IEEE 1364-2001 Annex A lacks (end labels, empty
// argument lists, `[size]` dimensions, nested assignments and similar).
// Error-recovery kinds stay admitted; the frontend already reports them.
class Verilog2001SyntaxProfile {
public:
  explicit Verilog2001SyntaxProfile(Capture& capture) : capture(capture) {}

  // Depth is bounded by the parser's own recursion limit.
  void walk(const syntax::SyntaxNode& node) {
    if (!verilog2001SyntaxKind(node.kind)) {
      report(node.sourceRange(), kindLabel(node.kind));
      return;
    }
    checkShape(node);
    for (size_t i = 0, count = node.getChildCount(); i < count; i++) {
      if (const syntax::SyntaxNode* child = node.childNode(i))
        walk(*child);
    }
  }

private:
  using SK = syntax::SyntaxKind;

  // IEEE 1364-2001 Annex A productions as frontend syntax kinds.
  static bool verilog2001SyntaxKind(SK kind) {
    switch (kind) {
      // A.1 source text, configurations and library maps (Clause 13).
      case SK::CompilationUnit:
      case SK::ModuleDeclaration:
      case SK::ModuleHeader:
      case SK::ParameterPortList:
      case SK::AnsiPortList:
      case SK::NonAnsiPortList:
      case SK::ImplicitNonAnsiPort:
      case SK::ExplicitNonAnsiPort:
      case SK::EmptyNonAnsiPort:
      case SK::PortReference:
      case SK::PortConcatenation:
      case SK::ImplicitAnsiPort:
      case SK::NetPortHeader:
      case SK::VariablePortHeader:
      case SK::PortDeclaration:
      case SK::ConfigDeclaration:
      case SK::ConfigCellIdentifier:
      case SK::ConfigInstanceIdentifier:
      case SK::ConfigLiblist:
      case SK::ConfigUseClause:
      case SK::CellConfigRule:
      case SK::InstanceConfigRule:
      case SK::DefaultConfigRule:
      case SK::LibraryMap:
      case SK::LibraryDeclaration:
      case SK::LibraryIncludeStatement:
      case SK::LibraryIncDirClause:
      case SK::FilePathSpec:
      // A.2 declarations.
      case SK::DataDeclaration:
      case SK::Declarator:
      case SK::NetDeclaration:
      case SK::EqualsValueClause:
      case SK::GenvarDeclaration:
      case SK::ParameterDeclaration:
      case SK::ParameterDeclarationStatement:
      case SK::ImplicitType:
      case SK::RegType:
      case SK::IntegerType:
      case SK::TimeType:
      case SK::RealType:
      case SK::RealTimeType:
      case SK::EventType:
      case SK::VariableDimension:
      case SK::RangeDimensionSpecifier:
      case SK::DriveStrength:
      case SK::PullStrength:
      case SK::ChargeStrength:
      case SK::Delay3:
      case SK::DelayControl:
      case SK::FunctionDeclaration:
      case SK::TaskDeclaration:
      case SK::FunctionPrototype:
      case SK::FunctionPortList:
      case SK::FunctionPort:
      // A.3 primitive instances, A.4 module instances and generates.
      case SK::PrimitiveInstantiation:
      case SK::HierarchyInstantiation:
      case SK::HierarchicalInstance:
      case SK::InstanceName:
      case SK::ParameterValueAssignment:
      case SK::OrderedParamAssignment:
      case SK::NamedParamAssignment:
      case SK::OrderedPortConnection:
      case SK::NamedPortConnection:
      case SK::EmptyPortConnection:
      case SK::DefParam:
      case SK::DefParamAssignment:
      case SK::GenerateRegion:
      case SK::LoopGenerate:
      case SK::IfGenerate:
      case SK::CaseGenerate:
      case SK::GenerateBlock:
      // A.5 UDPs.
      case SK::UdpDeclaration:
      case SK::AnsiUdpPortList:
      case SK::NonAnsiUdpPortList:
      case SK::UdpBody:
      case SK::UdpEntry:
      case SK::UdpInitialStmt:
      case SK::UdpInputPortDecl:
      case SK::UdpOutputPortDecl:
      case SK::UdpSimpleField:
      case SK::UdpEdgeField:
      // A.6 behavioral statements.
      case SK::ContinuousAssign:
      case SK::InitialBlock:
      case SK::AlwaysBlock:
      case SK::SequentialBlockStatement:
      case SK::ParallelBlockStatement:
      case SK::NamedBlockClause:
      case SK::ConditionalStatement:
      case SK::ConditionalPredicate:
      case SK::ConditionalPattern:
      case SK::ElseClause:
      case SK::CaseStatement:
      case SK::StandardCaseItem:
      case SK::DefaultCaseItem:
      case SK::LoopStatement:
      case SK::ForeverStatement:
      case SK::ForLoopStatement:
      case SK::TimingControlStatement:
      case SK::EventControl:
      case SK::EventControlWithExpression:
      case SK::ImplicitEventControl:
      case SK::RepeatedEventControl:
      case SK::SignalEventExpression:
      case SK::BinaryEventExpression:
      case SK::ParenthesizedEventExpression:
      case SK::ExpressionStatement:
      case SK::ProceduralAssignStatement:
      case SK::ProceduralForceStatement:
      case SK::ProceduralDeassignStatement:
      case SK::ProceduralReleaseStatement:
      case SK::DisableStatement:
      case SK::BlockingEventTriggerStatement:
      case SK::WaitStatement:
      case SK::EmptyStatement:
      // A.7 specify blocks.
      case SK::SpecifyBlock:
      case SK::SpecparamDeclaration:
      case SK::SpecparamDeclarator:
      case SK::PathDeclaration:
      case SK::PathDescription:
      case SK::SimplePathSuffix:
      case SK::EdgeSensitivePathSuffix:
      case SK::ConditionalPathDeclaration:
      case SK::IfNonePathDeclaration:
      case SK::PulseStyleDeclaration:
      case SK::SystemTimingCheck:
      case SK::TimingCheckEventArg:
      case SK::TimingCheckEventCondition:
      case SK::ExpressionTimingCheckArg:
      case SK::EmptyTimingCheckArg:
      case SK::EdgeControlSpecifier:
      case SK::EdgeDescriptor:
      // A.8 expressions and A.9 general.
      case SK::AttributeInstance:
      case SK::AttributeSpec:
      case SK::IdentifierName:
      case SK::IdentifierSelectName:
      case SK::ScopedName:
      case SK::SystemName:
      case SK::IntegerLiteralExpression:
      case SK::IntegerVectorExpression:
      case SK::RealLiteralExpression:
      case SK::StringLiteralExpression:
      case SK::UnaryPlusExpression:
      case SK::UnaryMinusExpression:
      case SK::UnaryLogicalNotExpression:
      case SK::UnaryBitwiseNotExpression:
      case SK::UnaryBitwiseAndExpression:
      case SK::UnaryBitwiseNandExpression:
      case SK::UnaryBitwiseOrExpression:
      case SK::UnaryBitwiseNorExpression:
      case SK::UnaryBitwiseXorExpression:
      case SK::UnaryBitwiseXnorExpression:
      case SK::AddExpression:
      case SK::SubtractExpression:
      case SK::MultiplyExpression:
      case SK::DivideExpression:
      case SK::ModExpression:
      case SK::PowerExpression:
      case SK::EqualityExpression:
      case SK::InequalityExpression:
      case SK::CaseEqualityExpression:
      case SK::CaseInequalityExpression:
      case SK::LessThanExpression:
      case SK::LessThanEqualExpression:
      case SK::GreaterThanExpression:
      case SK::GreaterThanEqualExpression:
      case SK::LogicalAndExpression:
      case SK::LogicalOrExpression:
      case SK::BinaryAndExpression:
      case SK::BinaryOrExpression:
      case SK::BinaryXorExpression:
      case SK::BinaryXnorExpression:
      case SK::LogicalShiftLeftExpression:
      case SK::LogicalShiftRightExpression:
      case SK::ArithmeticShiftLeftExpression:
      case SK::ArithmeticShiftRightExpression:
      case SK::ConditionalExpression:
      case SK::ConcatenationExpression:
      case SK::MultipleConcatenationExpression:
      case SK::ParenthesizedExpression:
      case SK::MinTypMaxExpression:
      case SK::ElementSelectExpression:
      case SK::ElementSelect:
      case SK::BitSelect:
      case SK::SimpleRangeSelect:
      case SK::AscendingRangeSelect:
      case SK::DescendingRangeSelect:
      case SK::MemberAccessExpression:
      case SK::InvocationExpression:
      case SK::ArgumentList:
      case SK::OrderedArgument:
      case SK::EmptyArgument:
      case SK::AssignmentExpression:
      case SK::NonblockingAssignmentExpression:
      case SK::TimingControlExpression:
      // Port connections and arguments are parsed as property expressions;
      // these wrappers hold a plain expression.
      case SK::SimplePropertyExpr:
      case SK::SimpleSequenceExpr:
      case SK::ParenthesizedPropertyExpr:
      case SK::ParenthesizedSequenceExpr:
      // Error recovery; the frontend reports these itself.
      case SK::Unknown:
      case SK::BadExpression:
      case SK::EmptyIdentifierName:
        return true;
      default:
        return false;
    }
  }

  static std::string kindLabel(SK kind) {
    switch (kind) {
      case SK::NamedLabel: return "statement label";
      case SK::WildcardPortConnection: return "implicit named port connection";
      case SK::ExplicitAnsiPort: return "explicit ANSI port expression";
      case SK::NamedArgument: return "named subroutine argument";
      case SK::UnbasedUnsizedLiteralExpression: return "unbased unsized literal";
      case SK::TimeLiteralExpression: return "time literal";
      case SK::CastExpression:
      case SK::SignedCastExpression: return "cast";
      case SK::EmptyMember: return "empty module item";
      default: break;
    }
    // A Slang kind spelling such as `AssignmentPatternExpression` becomes
    // `assignment pattern expression`.
    const std::string_view name = syntax::toString(kind);
    std::string label;
    for (size_t i = 0; i < name.size(); i++) {
      const char c = name[i];
      const bool upper = c >= 'A' && c <= 'Z';
      if (upper && i != 0 &&
          ((name[i - 1] >= 'a' && name[i - 1] <= 'z') ||
           (i + 1 < name.size() && name[i + 1] >= 'a' && name[i + 1] <= 'z' &&
            name[i - 1] >= 'A' && name[i - 1] <= 'Z')))
        label += ' ';
      label += upper ? static_cast<char>(c - 'A' + 'a') : c;
    }
    return label;
  }

  static bool isEmptyImplicitType(const syntax::DataTypeSyntax* type) {
    if (!type)
      return true;
    if (type->kind != SK::ImplicitType)
      return false;
    const auto& implicit = type->as<syntax::ImplicitTypeSyntax>();
    return !implicit.signing && implicit.dimensions.empty();
  }

  // 1364-2001 A.2.1.1 / A.2.6: parameter, function result and task/function
  // argument types are `[signed] [range]`, integer, real, realtime or time.
  static bool scalarTypeKind(SK kind) {
    return kind == SK::ImplicitType || kind == SK::IntegerType ||
           kind == SK::TimeType || kind == SK::RealType ||
           kind == SK::RealTimeType;
  }

  void checkShape(const syntax::SyntaxNode& node) {
    switch (node.kind) {
      case SK::NamedBlockClause:
        checkNamedBlockClause(node);
        break;
      case SK::ModuleHeader: {
        const auto& header = node.as<syntax::ModuleHeaderSyntax>();
        if (header.lifetime)
          report(header.lifetime.range(), "module lifetime");
        break;
      }
      case SK::ParameterPortList: {
        const auto& list = node.as<syntax::ParameterPortListSyntax>();
        bool first = true;
        for (const auto* declaration : list.declarations) {
          // A.1.3: every port declaration starts with `parameter`; a later
          // declaration without a keyword continues the previous list.
          if (first && !declaration->keyword)
            report(declaration->sourceRange(),
                   "parameter port without the parameter keyword");
          if (declaration->keyword &&
              declaration->keyword.kind != parsing::TokenKind::ParameterKeyword)
            report(declaration->keyword.range(),
                   "localparam in a parameter port list");
          first = false;
        }
        break;
      }
      case SK::ParameterDeclaration: {
        const auto& declaration = node.as<syntax::ParameterDeclarationSyntax>();
        if (!scalarTypeKind(declaration.type->kind))
          report(declaration.type->sourceRange(), "parameter data type");
        break;
      }
      case SK::DataDeclaration: {
        const auto& declaration = node.as<syntax::DataDeclarationSyntax>();
        if (!declaration.modifiers.empty())
          report(declaration.modifiers[0].range(), "declaration qualifier");
        break;
      }
      case SK::NetDeclaration: {
        const auto& declaration = node.as<syntax::NetDeclarationSyntax>();
        if (declaration.type->kind != SK::ImplicitType)
          report(declaration.type->sourceRange(), "net data type");
        break;
      }
      case SK::VariableDimension: {
        const auto& dimension = node.as<syntax::VariableDimensionSyntax>();
        // A.2.5: dimension ::= [ expr : expr ].
        if (!dimension.specifier)
          report(node.sourceRange(), "dynamic array dimension");
        else if (dimension.specifier->kind == SK::RangeDimensionSpecifier &&
                 dimension.specifier->as<syntax::RangeDimensionSpecifierSyntax>()
                         .selector->kind != SK::SimpleRangeSelect)
          report(node.sourceRange(), "unpacked dimension size");
        break;
      }
      case SK::FunctionPrototype:
        checkPrototype(node.as<syntax::FunctionPrototypeSyntax>());
        break;
      case SK::FunctionPort: {
        const auto& port = node.as<syntax::FunctionPortSyntax>();
        if (port.constKeyword || port.staticKeyword || port.varKeyword)
          report(node.sourceRange(), "argument qualifier");
        if (port.dataType && !scalarTypeKind(port.dataType->kind) &&
            port.dataType->kind != SK::RegType)
          report(port.dataType->sourceRange(), "argument data type");
        if (!port.direction && !isEmptyImplicitType(port.dataType))
          report(node.sourceRange(), "argument type without a direction");
        if (!port.declarator->dimensions.empty())
          report(port.declarator->dimensions[0]->sourceRange(),
                 "unpacked array argument");
        break;
      }
      case SK::InvocationExpression: {
        const auto& call = node.as<syntax::InvocationExpressionSyntax>();
        // A.8.2: user calls list expressions; system calls may leave
        // arguments empty (A.6.9 system_task_enable).
        if (call.left->kind == SK::SystemName || !call.arguments)
          break;
        if (call.arguments->parameters.empty())
          report(call.arguments->sourceRange(), "empty subroutine argument list");
        for (const auto* argument : call.arguments->parameters) {
          if (argument->kind == SK::EmptyArgument)
            report(call.arguments->sourceRange(), "empty subroutine argument");
        }
        break;
      }
      case SK::NamedPortConnection: {
        const auto& connection = node.as<syntax::NamedPortConnectionSyntax>();
        if (!connection.openParen)
          report(connection.name.range(), "implicit named port connection");
        break;
      }
      case SK::SignalEventExpression: {
        const auto& event = node.as<syntax::SignalEventExpressionSyntax>();
        // A.6.5 event_expression has only posedge and negedge.
        if (event.edge.kind == parsing::TokenKind::EdgeKeyword)
          report(event.edge.range(), "edge event control");
        break;
      }
      case SK::ConditionalPredicate:
        if (node.as<syntax::ConditionalPredicateSyntax>().conditions.size() > 1)
          report(node.sourceRange(), "conditional pattern list");
        break;
      case SK::ForLoopStatement: {
        const auto& loop = node.as<syntax::ForLoopStatementSyntax>();
        // A.6.8: one variable_assignment on each side of a condition.
        if (loop.initializers.size() != 1 || loop.steps.size() != 1 ||
            !loop.stopExpr)
          report(loop.forKeyword.range(), "SystemVerilog for-loop header");
        break;
      }
      case SK::LoopGenerate: {
        const auto& loop = node.as<syntax::LoopGenerateSyntax>();
        // A.4.2 genvar_assignment names an already declared genvar.
        if (loop.genvar)
          report(loop.genvar.range(), "genvar declaration in a generate loop");
        // 12.1.3.2: the loop body is `begin : name ... end`.
        if (loop.block->kind != SK::GenerateBlock ||
            !loop.block->as<syntax::GenerateBlockSyntax>().beginName)
          report(loop.block->sourceRange(), "unnamed generate loop block");
        break;
      }
      case SK::ConfigDeclaration:
        if (!node.as<syntax::ConfigDeclarationSyntax>().localparams.empty())
          report(node.sourceRange(), "configuration localparam");
        break;
      case SK::ConfigUseClause:
        if (node.as<syntax::ConfigUseClauseSyntax>().paramAssignments)
          report(node.sourceRange(), "configuration parameter assignment");
        break;
      case SK::AssignmentExpression:
      case SK::NonblockingAssignmentExpression:
        checkAssignmentPlacement(node);
        break;
      default:
        break;
    }
  }

  // A.6.3/A.4.2: only `begin : name`, `fork : name` and generate blocks
  // carry a name; no closing keyword takes a `: label`.
  void checkNamedBlockClause(const syntax::SyntaxNode& node) {
    const syntax::SyntaxNode* parent = node.parent;
    if (!parent)
      return;
    if (syntax::BlockStatementSyntax::isKind(parent->kind) &&
        parent->as<syntax::BlockStatementSyntax>().blockName == &node)
      return;
    if (parent->kind == SK::GenerateBlock &&
        parent->as<syntax::GenerateBlockSyntax>().beginName == &node)
      return;
    // Report from the closing keyword so the position matches the token
    // check for the same spelled form.
    SourceRange range = node.sourceRange();
    for (size_t i = 1, count = parent->getChildCount(); i < count; i++) {
      if (parent->childNode(i) == &node) {
        if (parsing::Token keyword = parent->childToken(i - 1))
          range = SourceRange(keyword.location(), range.end());
        break;
      }
    }
    report(range, "end label");
  }

  void checkPrototype(const syntax::FunctionPrototypeSyntax& prototype) {
    if (prototype.keyword.kind == parsing::TokenKind::FunctionKeyword &&
        !scalarTypeKind(prototype.returnType->kind))
      report(prototype.returnType->sourceRange(), "function return type");
    if (prototype.name->kind != SK::IdentifierName)
      report(prototype.name->sourceRange(), "qualified subroutine name");
    if (!prototype.portList)
      return;
    // A.2.6-A.2.7: an ANSI argument list is nonempty and starts with a
    // direction.
    const auto& ports = prototype.portList->ports;
    if (ports.empty()) {
      report(prototype.portList->sourceRange(), "empty subroutine argument list");
      return;
    }
    const auto* first = ports[0];
    if (first->kind == SK::FunctionPort &&
        !first->as<syntax::FunctionPortSyntax>().direction)
      report(first->sourceRange(), "argument without a direction");
  }

  // A.6.2/A.6.8/A.6.1: assignments are statements, continuous assignments,
  // for-loop/generate-loop steps or procedural continuous assignments,
  // never operands of an expression.
  void checkAssignmentPlacement(const syntax::SyntaxNode& node) {
    const syntax::SyntaxNode* parent = node.parent;
    if (!parent)
      return;
    switch (parent->kind) {
      case SK::ExpressionStatement:
      case SK::ContinuousAssign:
      case SK::ForLoopStatement:
      case SK::LoopGenerate:
      case SK::ProceduralAssignStatement:
      case SK::ProceduralForceStatement:
        if (node.kind == SK::AssignmentExpression ||
            parent->kind == SK::ExpressionStatement)
          return;
        break;
      default:
        break;
    }
    report(node.sourceRange(), "assignment within an expression");
  }

  void report(SourceRange range, std::string_view label) {
    if (capture.output.edition_findings.size() >= LLG_SLANG_MAX_EDITION_FINDINGS)
      return;
    const LlgSlangSourceRange span = capture.span(range);
    if (span.file_id == LLG_SLANG_INVALID_ID)
      return;
    chargeRecord(capture.output, sizeof(LlgSlangEditionFinding));
    capture.output.edition_findings.push_back(
        {span, storeString(capture.output, label)});
  }

  Capture& capture;
};

// The checker declarations that have an elaborated instance below `root`.
// Built only when the unused-declaration check reports a checker.
std::unordered_set<const Symbol*> collectInstantiatedCheckers(const RootSymbol& root) {
  std::unordered_set<const Symbol*> result;
  root.visit(makeVisitor([&](auto& visitor, const CheckerInstanceSymbol& instance) {
    result.insert(&instance.body.checker);
    visitor.visitDefault(instance);
  }));
  return result;
}

// Whether `symbol` is declared in the body of a checker instance that is not
// procedural (IEEE 1800-2009 17.3), at any nesting depth.
bool inStaticCheckerBody(const Symbol* symbol) {
  for (const Scope* scope = symbol ? symbol->getParentScope() : nullptr; scope;
       scope = scope->asSymbol().getParentScope()) {
    const Symbol& owner = scope->asSymbol();
    if (owner.kind == SymbolKind::CheckerInstanceBody)
      return !owner.as<CheckerInstanceBodySymbol>().isProcedural;
    if (owner.kind == SymbolKind::InstanceBody)
      return false;
  }
  return false;
}

class CaptureClient final : public DiagnosticClient {
public:
  CaptureClient(Capture& capture, uint32_t provider)
      : capture(capture), provider(provider) {}

  void issue(DiagnosticEngine& diagnosticEngine, const Diagnostic& diagnostic) {
    root = &diagnostic;
    rootIndex = LLG_SLANG_INVALID_ID;
    if (diagnostic.isError() && diagnostic.code != diag::AlwaysWithoutTimingControl &&
        diagnostic.code != diag::AlwaysInChecker)
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

// ---------------------------------------------------------------------------
// Type-directed top-level parameter overrides.
//
// Slang parses each `NAME=VALUE` override as an expression and converts it to
// the parameter's type. That is exact for most types, but a string parameter
// cannot take bare text, an enum member is only visible where its type is
// declared, an unsized decimal of 2^31 or more is rejected, a type parameter
// needs a type rather than an expression and a local parameter must not be
// overridden. The bridge therefore classifies each target before compiling,
// using default instances of the top definitions in a throwaway compilation
// that shares the parsed trees, and rewrites the VALUE text accordingly.


std::string_view trimmed(std::string_view text) {
  const auto first = text.find_first_not_of(" \t\r\n\f\v");
  if (first == std::string_view::npos)
    return {};
  const auto last = text.find_last_not_of(" \t\r\n\f\v");
  return text.substr(first, last - first + 1);
}

bool isSimpleIdentifier(std::string_view text) {
  if (text.empty() || !(std::isalpha(static_cast<unsigned char>(text[0])) || text[0] == '_'))
    return false;
  return std::all_of(text.begin() + 1, text.end(), [](char c) {
    return std::isalnum(static_cast<unsigned char>(c)) || c == '_' || c == '$';
  });
}

// A complete double-quoted SystemVerilog string literal: no unescaped quote
// before the closing one.
bool isQuotedStringLiteral(std::string_view text) {
  if (text.size() < 2 || text.front() != '"' || text.back() != '"')
    return false;
  for (size_t i = 1; i + 1 < text.size(); i++) {
    if (text[i] == '\\')
      i++;
    else if (text[i] == '"')
      return false;
  }
  // The closing quote must not itself be escaped.
  size_t backslashes = 0;
  for (size_t i = text.size() - 1; i > 1 && text[i - 1] == '\\'; i--)
    backslashes++;
  return backslashes % 2 == 0;
}

// A string literal whose value is exactly the bytes of `text`.
std::string quotedStringLiteral(std::string_view text) {
  std::string result = "\"";
  for (char raw : text) {
    const auto c = static_cast<unsigned char>(raw);
    if (c == '\\' || c == '"') {
      result.push_back('\\');
      result.push_back(raw);
    }
    else if (c < 0x20 || c == 0x7f) {
      result.push_back('\\');
      result.push_back(static_cast<char>('0' + ((c >> 6) & 7)));
      result.push_back(static_cast<char>('0' + ((c >> 3) & 7)));
      result.push_back(static_cast<char>('0' + (c & 7)));
    }
    else {
      result.push_back(raw);
    }
  }
  result.push_back('"');
  return result;
}

// Decimal digits (with `_` separators) longer than this are left to Slang;
// the conversion below is quadratic in the digit count.
constexpr size_t kMaxNormalizedDecimalDigits = 100000;

// Bits needed for the unsigned value of decimal `digits`, or 0 for zero.
uint64_t decimalBitLength(std::string_view digits) {
  std::vector<uint32_t> limbs; // little-endian base 2^32
  for (char c : digits) {
    if (c == '_')
      continue;
    uint64_t carry = static_cast<uint64_t>(c - '0');
    for (uint32_t& limb : limbs) {
      const uint64_t product = static_cast<uint64_t>(limb) * 10 + carry;
      limb = static_cast<uint32_t>(product);
      carry = product >> 32;
    }
    if (carry != 0)
      limbs.push_back(static_cast<uint32_t>(carry));
  }
  while (!limbs.empty() && limbs.back() == 0)
    limbs.pop_back();
  if (limbs.empty())
    return 0;
  return (limbs.size() - 1) * 32 + static_cast<uint64_t>(std::bit_width(limbs.back()));
}

// An unsized decimal literal (optionally signed) is signed and at least 32
// bits wide; Slang rejects one of 2^31 or more as an override. Give such a
// value an explicit width (its bits plus a sign bit) so it keeps its exact
// value and signedness: `-4294967296` becomes `-34'sd4294967296`.
std::string normalizedDecimalOverride(std::string_view value) {
  const std::string_view text = trimmed(value);
  size_t start = 0;
  if (!text.empty() && (text[0] == '-' || text[0] == '+'))
    start = 1;
  const std::string_view digits = trimmed(text.substr(start));
  if (digits.empty() || !std::isdigit(static_cast<unsigned char>(digits[0])) ||
      digits.size() > kMaxNormalizedDecimalDigits ||
      !std::all_of(digits.begin(), digits.end(), [](char c) {
        return std::isdigit(static_cast<unsigned char>(c)) || c == '_';
      }))
    return std::string(value);
  const uint64_t bits = decimalBitLength(digits);
  if (bits < 32)
    return std::string(value);
  std::string result(text.substr(0, start));
  result += std::to_string(bits + 1);
  result += "'sd";
  result += digits;
  return result;
}

// Declared type syntax is the plain implicit type: no `signed` and no packed
// dimensions, so the parameter takes the type of its value.
bool isUntypedParameter(const ParameterSymbol& parameter) {
  const DeclaredType* declared = parameter.getDeclaredType();
  const syntax::DataTypeSyntax* syntax = declared ? declared->getTypeSyntax() : nullptr;
  if (!syntax || syntax->kind != syntax::SyntaxKind::ImplicitType)
    return false;
  const auto& implicit = syntax->as<syntax::ImplicitTypeSyntax>();
  return !implicit.signing && implicit.dimensions.empty();
}

std::string enumMemberOverride(const InstanceBodySymbol& body,
                               const ParameterSymbol& parameter,
                               const Type& canonical, std::string_view value) {
  const std::string_view name = trimmed(value);
  // A name visible in the instance keeps its ordinary expression meaning.
  if (!isSimpleIdentifier(name) || Lookup::unqualified(body, name) != nullptr)
    return std::string(value);
  const DeclaredType* declared = parameter.getDeclaredType();
  const syntax::DataTypeSyntax* typeSyntax = declared ? declared->getTypeSyntax() : nullptr;
  if (!typeSyntax || typeSyntax->kind != syntax::SyntaxKind::NamedType)
    return std::string(value);
  for (const EnumValueSymbol& member : canonical.as<EnumType>().values()) {
    if (member.name != name)
      continue;
    const ConstantValue& constant = member.getValue();
    if (!constant.isInteger())
      break;
    // The declared type name resolves at the parameter, so a cast of the
    // member's exact value selects that member.
    return std::string(trimmed(typeSyntax->toString())) + "'(" +
           constant.integer().toString(LiteralBase::Binary, true) + ")";
  }
  return std::string(value);
}

struct ParameterOverrideRequest {
  std::string name;
  std::string value;
};

struct ParameterOverridePlan {
  // NAME=VALUE strings handed to Slang.
  std::vector<std::string> overrides;
  // Rewritten spelling -> given spelling, for options and for values.
  std::unordered_map<std::string, std::string> givenOptions;
  std::unordered_map<std::string, std::string> givenValues;
  // Overrides rejected before compilation, reported as error diagnostics.
  std::vector<std::string> localErrors;
};

// Top definitions Slang will elaborate: the requested tops, or else the
// outermost modules and programs of non-library default-library units that
// nothing instantiates.
std::vector<const DefinitionSymbol*> overrideTopDefinitions(
    const Compilation& probe,
    const std::vector<std::shared_ptr<syntax::SyntaxTree>>& trees,
    const decltype(CompilationOptions::topModules)& topModules) {
  std::vector<const DefinitionSymbol*> result;
  const RootSymbol& root = probe.getRootNoFinalize();
  auto add = [&](std::string_view name) {
    const Symbol* symbol = probe.tryGetDefinition(name, root).definition;
    if (!symbol || symbol->kind != SymbolKind::Definition)
      return false;
    const auto* definition = &symbol->as<DefinitionSymbol>();
    if (std::find(result.begin(), result.end(), definition) == result.end())
      result.push_back(definition);
    return true;
  };
  if (!topModules.empty()) {
    for (std::string_view top : topModules) {
      const std::string_view name = top;
      if (!add(name) && name.find('.') != std::string_view::npos)
        add(name.substr(name.rfind('.') + 1));
    }
    return result;
  }
  std::unordered_set<std::string_view> instantiated;
  for (const auto& tree : trees) {
    for (const auto* instance : tree->getMetadata().globalInstances)
      instantiated.emplace(instance->type.valueText());
  }
  for (const auto& tree : trees) {
    const SourceLibrary* library = tree->getSourceLibrary();
    if (tree->isLibraryUnit || (library && !library->isDefault))
      continue;
    for (const auto& [declaration, _] : tree->getMetadata().nodeMeta) {
      if (!declaration->parent ||
          declaration->parent->kind != syntax::SyntaxKind::CompilationUnit ||
          (declaration->kind != syntax::SyntaxKind::ModuleDeclaration &&
           declaration->kind != syntax::SyntaxKind::ProgramDeclaration))
        continue;
      const std::string_view name = declaration->header->name.valueText();
      if (!name.empty() && !instantiated.contains(name))
        add(name);
    }
  }
  return result;
}

ParameterOverridePlan planParameterOverrides(
    const std::vector<ParameterOverrideRequest>& requests,
    const std::vector<std::shared_ptr<syntax::SyntaxTree>>& trees,
    const CompilationOptions& options, const SourceLibrary& defaultLibrary) {
  ParameterOverridePlan plan;
  CompilationOptions probeOptions = options;
  probeOptions.paramOverrides.clear();
  Bag probeBag;
  probeBag.set(probeOptions);
  // Shares the parsed trees; only default instances are created, and nothing
  // from this compilation outlives the function.
  Compilation probe(probeBag, &defaultLibrary);
  for (const auto& tree : trees)
    probe.addSyntaxTree(tree);
  std::vector<const InstanceSymbol*> tops;
  for (const DefinitionSymbol* definition :
       overrideTopDefinitions(probe, trees, options.topModules))
    tops.push_back(&InstanceSymbol::createDefault(probe, *definition));

  for (const ParameterOverrideRequest& request : requests) {
    std::string value = request.value;
    const ParameterSymbolBase* target = nullptr;
    const InstanceSymbol* owner = nullptr;
    for (const InstanceSymbol* top : tops) {
      for (const ParameterSymbolBase* parameter : top->body.getParameters()) {
        const std::string_view name = parameter->symbol.name;
        if (request.name == name || matchesQualifiedParameter(request.name, top->name, name) ||
            matchesQualifiedParameter(request.name, top->getDefinition().name, name)) {
          target = parameter;
          owner = top;
          break;
        }
      }
      if (target)
        break;
    }
    if (target && target->isLocalParam()) {
      plan.localErrors.push_back("parameter override '" + request.name +
                                 "' targets local parameter '" +
                                 std::string(target->symbol.name) + "' of '" +
                                 std::string(owner->getDefinition().name) +
                                 "', which cannot be overridden");
      continue;
    }
    if (target && target->symbol.kind == SymbolKind::TypeParameter) {
      // A type, spelled as in a parameter value assignment.
      const std::string_view text = trimmed(value);
      if (!text.starts_with("type(") && !text.starts_with("type ("))
        value = "type(" + std::string(text) + ")";
    }
    else if (target && target->symbol.kind == SymbolKind::Parameter) {
      const auto& parameter = target->symbol.as<ParameterSymbol>();
      const Type& canonical = parameter.getType().getCanonicalType();
      if (canonical.isString() && !isUntypedParameter(parameter)) {
        if (!isQuotedStringLiteral(trimmed(value)))
          value = quotedStringLiteral(value);
      }
      else if (canonical.isEnum()) {
        value = enumMemberOverride(owner->body, parameter, canonical, value);
      }
      else {
        value = normalizedDecimalOverride(value);
      }
    }
    else {
      // Unknown names are reported after elaboration; still normalize the
      // spelling so a valid value is not rejected for its width alone.
      value = normalizedDecimalOverride(value);
    }
    if (value != request.value) {
      plan.givenOptions.emplace(request.name + "=" + value,
                                request.name + "=" + request.value);
      plan.givenValues.emplace(value, request.value);
    }
    plan.overrides.push_back(request.name + "=" + value);
  }
  return plan;
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

std::unique_ptr<CaptureOutput> compileImpl(const LlgSlangCompileRequest& request) {
  if (request.abi_version != LLG_SLANG_ABI_VERSION)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unsupported Slang ABI version");
  constexpr uint32_t knownFlags =
      LLG_SLANG_COMPILE_LIBRARY_UNITS |
      LLG_SLANG_COMPILE_EDITION_VERILOG_2001 |
      LLG_SLANG_COMPILE_EDITION_SYSTEMVERILOG_2009 |
      LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS |
      LLG_SLANG_COMPILE_DELAY_SELECT_MIN |
      LLG_SLANG_COMPILE_DELAY_SELECT_MAX;
  if ((request.flags & ~knownFlags) != 0)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unknown compile request flags");
  constexpr uint32_t delaySelectFlags =
      LLG_SLANG_COMPILE_DELAY_SELECT_MIN | LLG_SLANG_COMPILE_DELAY_SELECT_MAX;
  if ((request.flags & delaySelectFlags) == delaySelectFlags)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "compile request selects both min and max delays");
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
  if (request.library_source_count != 0 && request.library_sources == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "library_sources has a null pointer");
  if (request.library_include_dir_count != 0 && request.library_include_dirs == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "library_include_dirs has a null pointer");
  if (request.library_order_count != 0 && request.library_order == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "library_order has a null pointer");
  if (request.default_library_present > 1 || request.default_library_reserved != 0)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "invalid default library flags");
  if (request.limits.max_semantic_edges > kHardMaxSemanticEdges)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "max_semantic_edges exceeds the native hard ceiling");

  const uint64_t maxSources = effectiveLimit(request.limits.max_sources,
      kDefaultMaxSources, kHardMaxSources);
  const uint64_t maxSourceBytes = effectiveLimit(request.limits.max_source_bytes,
      kDefaultMaxSourceBytes, kHardMaxSourceBytes);
  if (request.source_count > maxSources ||
      request.library_source_count > maxSources -
          std::min(request.source_count, maxSources))
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "source count limit exceeded");
  if (request.define_count > kHardMaxDefines)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "define count limit exceeded");
  if (request.top_module_count > kHardMaxTopModules)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "top module count limit exceeded");
  if (request.include_dir_count > kHardMaxIncludeDirs)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "include directory count limit exceeded");
  if (request.library_include_dir_count > kHardMaxIncludeDirs)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "library include directory count limit exceeded");
  if (request.parameter_override_count > kHardMaxParameterOverrides)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "parameter override count limit exceeded");
  if (request.system_subroutine_count > kHardMaxSystemSubroutines)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "system subroutine count limit exceeded");
  if (request.library_order_count > maxSources)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "library order count limit exceeded");

  uint64_t sourceBytes = 0;
  uint64_t configBytes = 0;
  std::vector<std::string> sourceNames;
  sourceNames.reserve(static_cast<size_t>(request.source_count + request.library_source_count));
  std::vector<std::string> sourcePaths;
  sourcePaths.reserve(static_cast<size_t>(request.source_count + request.library_source_count));
  // Normalized path -> input index, so uniqueness checks and buffer-to-file
  // association stay linear in the number of admitted sources.
  std::unordered_map<std::string, uint64_t> sourcePathIndex;
  sourcePathIndex.reserve(static_cast<size_t>(request.source_count + request.library_source_count));
  std::vector<std::string_view> sourceTexts;
  sourceTexts.reserve(static_cast<size_t>(request.source_count + request.library_source_count));
  for (uint64_t i = 0; i < request.source_count; i++) {
    const auto& input = request.sources[i];
    if ((input.flags & ~(LLG_SLANG_SOURCE_COMPILATION_UNIT | LLG_SLANG_SOURCE_LIBRARY_MAP)) != 0 ||
        ((input.flags & LLG_SLANG_SOURCE_LIBRARY_MAP) != 0 &&
         (input.flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) == 0) || input.reserved != 0)
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
    if (!sourcePathIndex.emplace(normalized, sourcePaths.size()).second)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "source names must be unique after lexical normalization");
    sourceNames.emplace_back(name);
    sourcePaths.push_back(normalized);
    sourceTexts.push_back(text);
  }
  std::vector<std::string> libraryNames;
  std::vector<std::string_view> librarySourceTexts;
  libraryNames.reserve(static_cast<size_t>(request.library_source_count));
  librarySourceTexts.reserve(static_cast<size_t>(request.library_source_count));
  for (uint64_t i = 0; i < request.library_source_count; i++) {
    const auto& input = request.library_sources[i];
    if ((input.flags & ~LLG_SLANG_LIBRARY_SOURCE_MAP) != 0 || input.reserved != 0)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "unknown library source flags or nonzero reserved field");
    const std::string_view name = checkedView(input.name, "library source name");
    const std::string_view text = checkedView(input.text, "library source text");
    const std::string_view library = checkedView(input.library, "library name");
    if (name.empty() || name.find('\0') != std::string_view::npos ||
        library.empty() || library.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "library source names and libraries must be nonempty and contain no NUL bytes");
    addChecked(sourceBytes, name.size(), maxSourceBytes, "source byte");
    addChecked(sourceBytes, text.size(), maxSourceBytes, "source byte");
    addChecked(sourceBytes, library.size(), maxSourceBytes, "source byte");
    const std::string normalized =
        std::filesystem::path(name).lexically_normal().generic_string();
    if (!sourcePathIndex.emplace(normalized, sourcePaths.size()).second)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "source names must be unique after lexical normalization");
    sourceNames.emplace_back(name);
    sourcePaths.push_back(normalized);
    sourceTexts.push_back(text);
    libraryNames.emplace_back(library);
    librarySourceTexts.push_back(text);
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
  if ((request.flags & LLG_SLANG_COMPILE_DELAY_SELECT_MIN) != 0)
    compilationOptions.minTypMax = MinTypMax::Min;
  else if ((request.flags & LLG_SLANG_COMPILE_DELAY_SELECT_MAX) != 0)
    compilationOptions.minTypMax = MinTypMax::Max;
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

  std::vector<std::string> libraryOrder;
  libraryOrder.reserve(static_cast<size_t>(request.library_order_count));
  for (uint64_t i = 0; i < request.library_order_count; i++) {
    const std::string_view name = checkedView(request.library_order[i], "library name");
    if (name.empty() || name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "library names must be nonempty and contain no NUL bytes");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes, "configuration byte");
    libraryOrder.emplace_back(name);
  }
  std::string defaultLibraryName = "work";
  if (request.default_library_present != 0) {
    const std::string_view name = checkedView(request.default_library, "default library name");
    if (name.empty() || name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "default library name must be nonempty and contain no NUL bytes");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes, "configuration byte");
    defaultLibraryName.assign(name);
  }

  std::vector<ParameterOverrideRequest> overrideRequests;
  overrideRequests.reserve(static_cast<size_t>(request.parameter_override_count));
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
    overrideRequests.push_back({std::string(name), std::string(value)});
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

  SourceLibrary defaultLibrary(std::move(defaultLibraryName), std::numeric_limits<int>::max());
  defaultLibrary.isDefault = true;
  std::vector<std::unique_ptr<SourceLibrary>> sourceLibraries;
  std::unordered_map<std::string, SourceLibrary*> sourceLibraryByName;
  sourceLibraryByName.emplace(defaultLibrary.name, &defaultLibrary);
  auto libraryFor = [&](const std::string& name) -> SourceLibrary* {
    if (auto it = sourceLibraryByName.find(name); it != sourceLibraryByName.end())
      return it->second;
    const int priority = static_cast<int>(sourceLibraries.size());
    auto library = std::make_unique<SourceLibrary>(std::string(name), priority);
    SourceLibrary* result = library.get();
    sourceLibraries.push_back(std::move(library));
    sourceLibraryByName.emplace(name, result);
    return result;
  };
  std::vector<std::string> discoveredLibraryOrder;
  for (const std::string& name : libraryNames) {
    libraryFor(name);
    if (std::find(discoveredLibraryOrder.begin(), discoveredLibraryOrder.end(), name) ==
        discoveredLibraryOrder.end())
      discoveredLibraryOrder.push_back(name);
  }
  if (libraryOrder.empty())
    libraryOrder = discoveredLibraryOrder;
  for (const std::string& name : libraryOrder) {
    if (name == defaultLibrary.name)
      continue;
    libraryFor(name);
    compilationOptions.defaultLiblist.push_back(name);
  }
  for (uint64_t i = 0; i < request.library_include_dir_count; i++) {
    const auto& input = request.library_include_dirs[i];
    const std::string_view library = checkedView(input.library, "library include directory library");
    const std::string_view path = checkedView(input.path, "library include directory path");
    if (library.empty() || path.empty() || library.find('\0') != std::string_view::npos ||
        path.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "library include directories require nonempty names without NUL bytes");
    addChecked(configBytes, library.size(), kHardMaxConfigBytes, "configuration byte");
    addChecked(configBytes, path.size(), kHardMaxConfigBytes, "configuration byte");
    libraryFor(std::string(library))->includeDirs.emplace_back(
        std::filesystem::path(path).lexically_normal());
  }

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
  std::vector<SourceBuffer> libraryBuffers;
  libraryBuffers.reserve(static_cast<size_t>(request.library_source_count));
  std::vector<std::pair<SourceLibrary*, std::vector<SourceBuffer>>> libraryBufferGroups;
  std::vector<SourceBuffer> libraryMapBuffers;
  for (uint64_t i = 0; i < request.library_source_count; i++) {
    auto* library = libraryFor(libraryNames[static_cast<size_t>(i)]);
    auto buffer = sourceManager.assignText(
        sourcePaths[static_cast<size_t>(request.source_count + i)],
        librarySourceTexts[static_cast<size_t>(i)], SourceLocation(), library);
    sourceManager.setBufferKind(buffer.id, SourceManager::BufferKind::LibraryFile);
    libraryBuffers.push_back(buffer);
    if ((request.library_sources[i].flags & LLG_SLANG_LIBRARY_SOURCE_MAP) != 0) {
      libraryMapBuffers.push_back(buffer);
      continue;
    }
    auto group = std::find_if(libraryBufferGroups.begin(), libraryBufferGroups.end(),
                              [library](const auto& entry) { return entry.first == library; });
    if (group == libraryBufferGroups.end()) {
      libraryBufferGroups.emplace_back(library, std::vector<SourceBuffer>{});
      group = std::prev(libraryBufferGroups.end());
    }
    group->second.push_back(buffer);
  }

  parsing::PreprocessorOptions preprocessorOptions;
  preprocessorOptions.languageVersion = edition.languageVersion;
  preprocessorOptions.predefines = std::move(predefines);
  preprocessorOptions.additionalIncludePaths = std::move(includeDirs);
  preprocessorOptions.keywordMapping.reserve(sourcePaths.size());
  for (const std::string& path : sourcePaths)
    preprocessorOptions.keywordMapping.emplace_back(path, edition.keywordVersion);
  parsing::LexerOptions lexerOptions;
  lexerOptions.languageVersion = edition.languageVersion;
  // Verilog-XL `protected/`endprotected envelopes are lexed (the encoded text
  // is skipped like a pragma protect data block) instead of being reported as
  // unknown directives; `protect/`endprotect become ignored directives.
  // Protected envelopes are reported through the capture.
  lexerOptions.enableLegacyProtect = true;
  parsing::ParserOptions parserOptions;
  parserOptions.languageVersion = edition.languageVersion;
  Bag parseOptions;
  parseOptions.set(std::move(preprocessorOptions));
  parseOptions.set(std::move(lexerOptions));
  parseOptions.set(std::move(parserOptions));
  ProfileStage parseStage("slang.parse");
  // Trees are parsed first so parameter overrides can be planned against the
  // design before the compilation that elaborates it is created.
  std::vector<std::shared_ptr<syntax::SyntaxTree>> trees;
  auto addTree = [&](std::shared_ptr<syntax::SyntaxTree> tree) {
    trees.push_back(std::move(tree));
  };
  bool anyCompilationUnit = false;
  if ((request.flags & LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS) != 0) {
    std::vector<SourceBuffer> compilationBuffers;
    compilationBuffers.reserve(static_cast<size_t>(request.source_count));
    for (uint64_t i = 0; i < request.source_count; i++) {
      if ((request.sources[i].flags & LLG_SLANG_SOURCE_LIBRARY_MAP) != 0) {
        auto tree = syntax::SyntaxTree::fromLibraryMapBuffer(
            buffers[static_cast<size_t>(i)], sourceManager, parseOptions);
        addTree(std::move(tree));
        anyCompilationUnit = true;
      } else if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) != 0) {
        compilationBuffers.push_back(buffers[static_cast<size_t>(i)]);
      }
    }
    anyCompilationUnit |= !compilationBuffers.empty();
    if (!compilationBuffers.empty()) {
      auto tree = syntax::SyntaxTree::fromBuffers(
          compilationBuffers, sourceManager, parseOptions);
      if ((request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0)
        tree->isLibraryUnit = true;
      addTree(std::move(tree));
    }
  } else {
    for (uint64_t i = 0; i < request.source_count; i++) {
      if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) == 0)
        continue;
      anyCompilationUnit = true;
      auto tree = (request.sources[i].flags & LLG_SLANG_SOURCE_LIBRARY_MAP) != 0
          ? syntax::SyntaxTree::fromLibraryMapBuffer(
                buffers[static_cast<size_t>(i)], sourceManager, parseOptions)
          : syntax::SyntaxTree::fromBuffer(
                buffers[static_cast<size_t>(i)], sourceManager, parseOptions);
      if ((request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0)
        tree->isLibraryUnit = true;
      addTree(std::move(tree));
    }
  }
  // Library sources follow the selected compilation-unit mode: merged mode
  // shares one preprocessor and `$unit` per library in admission order, while
  // separate mode gives every library file its own, like any other source.
  const bool mergedUnits =
      (request.flags & LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS) != 0;
  for (auto& group : libraryBufferGroups) {
    if (mergedUnits) {
      auto tree = syntax::SyntaxTree::fromBuffers(group.second, sourceManager, parseOptions);
      tree->isLibraryUnit = true;
      addTree(std::move(tree));
      continue;
    }
    for (const SourceBuffer& buffer : group.second) {
      auto tree = syntax::SyntaxTree::fromBuffer(buffer, sourceManager, parseOptions);
      tree->isLibraryUnit = true;
      addTree(std::move(tree));
    }
  }
  for (auto& buffer : libraryMapBuffers) {
    auto tree = syntax::SyntaxTree::fromLibraryMapBuffer(buffer, sourceManager, parseOptions);
    tree->isLibraryUnit = true;
    addTree(std::move(tree));
  }
  if (!anyCompilationUnit)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "at least one compilation unit source is required");

  ParameterOverridePlan overridePlan;
  if (!overrideRequests.empty())
    overridePlan = planParameterOverrides(overrideRequests, trees, compilationOptions,
                                          defaultLibrary);
  compilationOptions.paramOverrides = overridePlan.overrides;
  Bag compileOptions;
  compileOptions.set(std::move(compilationOptions));
  Compilation compilation(compileOptions, &defaultLibrary);
  for (const auto& subroutine : userDefinedSubroutines)
    compilation.addSystemSubroutine(subroutine);
  for (auto& tree : trees)
    compilation.addSyntaxTree(std::move(tree));
  trees.clear();

  parseStage.finish();
  auto output = std::make_unique<CaptureOutput>();
  output->flags |= edition.snapshotFlag;
  if ((request.flags & LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS) != 0)
    output->flags |= LLG_SLANG_SNAPSHOT_MERGED_COMPILATION_UNITS;
  if ((request.flags & LLG_SLANG_COMPILE_DELAY_SELECT_MIN) != 0)
    output->flags |= LLG_SLANG_SNAPSHOT_DELAY_SELECT_MIN;
  if ((request.flags & LLG_SLANG_COMPILE_DELAY_SELECT_MAX) != 0)
    output->flags |= LLG_SLANG_SNAPSHOT_DELAY_SELECT_MAX;
  output->output_byte_limit = effectiveLimit(request.limits.max_output_bytes,
      kDefaultMaxOutputBytes, kHardMaxOutputBytes);
  Capture capture{*output, sourceManager, request.limits,
                  (request.flags & LLG_SLANG_COMPILE_LIBRARY_UNITS) != 0};
  capture.givenOverrideOptions = std::move(overridePlan.givenOptions);
  capture.givenOverrideValues = std::move(overridePlan.givenValues);
  const uint64_t totalSourceCount = request.source_count + request.library_source_count;
  for (uint64_t i = 0; i < totalSourceCount; i++) {
    chargeRecord(*output, sizeof(LlgSlangFile));
    output->files.push_back({i, storeString(*output, sourceNames[i]),
                             static_cast<uint64_t>(sourceTexts[i].size())});
    const auto buffer = i < request.source_count
                            ? buffers[static_cast<size_t>(i)]
                            : libraryBuffers[static_cast<size_t>(i - request.source_count)];
    capture.fileIds.emplace_back(buffer.id, i);
  }

  // A cached include gets a new BufferID that refers to the same admitted file.
  // Associate every physical buffer with its input file before copying ranges.
  for (BufferID buffer : sourceManager.getAllBuffers()) {
    if (!sourceManager.isFileLoc(SourceLocation(buffer, 0)))
      continue;
    const auto path = sourceManager.getFullPath(buffer).generic_string();
    if (const auto found = sourcePathIndex.find(path); found != sourcePathIndex.end())
      capture.fileIds.emplace_back(buffer, found->second);
  }

  // Capture source bodies before diagnostics are cached so their errors are included.
  if (capture.declarationOnly)
    captureNavigation(compilation, capture);
  ProfileStage elaborateStage("slang.elaborate");
  const Diagnostics& compilationDiagnostics = compilation.getAllDiagnostics();
  elaborateStage.finish();
  ProfileStage instanceStage("wrapper.instances");
  DiagnosticEngine engine(sourceManager);
  auto compilationClient = std::make_shared<CaptureClient>(
      capture, LLG_SLANG_DIAG_COMPILATION);
  engine.addClient(compilationClient);
  // IEEE 1800-2009 13.5.2 makes a `ref` formal of a static subroutine
  // illegal; the frontend only warns by default. Neither edition admits it
  // (1364-2001 has no `ref`).
  engine.setSeverity(diag::RefArgAutomaticFunc, DiagnosticSeverity::Error);
  // IEEE 1364-2001 9.3.2 and 1800-2009 10.6.2 admit only nets, whole
  // variables, constant net selects and their concatenations as force/release
  // targets. The frontend only warns (other tools' compatibility) but still
  // drops the statement, which would otherwise surface as an unlocated
  // lowering failure.
  engine.setSeverity(diag::BadProceduralForce, DiagnosticSeverity::Error);
  // IEEE 1364-2001 Table 8 and 1800-2009 Table 6-11: a parameter "May not be
  // used inside specify blocks"; the frontend only warns. Duplicate or
  // overlapping module paths stay a warning: each is an active path and the
  // smallest delay applies (1800-2009 30.5.3, decision A7-D4).
  engine.setSeverity(diag::SpecifyBlockParam, DiagnosticSeverity::Error);
  // IEEE 1800-2009 17.5 admits `always` procedures in a checker body
  // (checker_always_construct ::= always statement); the frontend follows
  // the later revision, which replaced them with always_comb/_latch/_ff.
  // The owned checker lowering enforces the 2009 statement restrictions.
  engine.setSeverity(diag::AlwaysInChecker, DiagnosticSeverity::Ignored);
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
  for (std::string& message : overridePlan.localErrors)
    capture.addIntegrationDiagnostic("LocalParameterOverride", std::move(message));
  for (const InstanceSymbol* top : root.topInstances)
    capture.instance(*top, LLG_SLANG_INVALID_ID);

  instanceStage.finish();
  {
    ProfileStage analysisStage("slang.analysis");
    FreezeGuard frozen(compilation);
    analysis::AnalysisOptions analysisOptions;
    analysisOptions.flags |= analysis::AnalysisFlags::CheckUnused |
                             analysis::AnalysisFlags::CheckShadow;
    analysis::AnalysisManager manager(analysisOptions);
    manager.analyze(compilation);
    Diagnostics diagnostics = manager.getDiagnostics();
    diagnostics.sort(sourceManager);
    std::optional<std::unordered_set<const Symbol*>> instantiatedCheckers;
    engine.clearClients();
    auto analysisClient = std::make_shared<CaptureClient>(
        capture, LLG_SLANG_DIAG_ANALYSIS);
    engine.addClient(analysisClient);
    engine.setSeverity(diag::AlwaysWithoutTimingControl, DiagnosticSeverity::Warning);
    for (const Diagnostic& diagnostic : diagnostics) {
      // The unused check does not count an export "DPI-C" declaration as a
      // use, but C code calls the exported subroutine (IEEE 1800-2009 35.7).
      if ((diagnostic.code == diag::UnusedSubroutine ||
           diagnostic.code == diag::UnusedPackageSubroutine) &&
          diagnostic.symbol && diagnostic.symbol->kind == SymbolKind::Subroutine &&
          dpiExportName(capture, diagnostic.symbol->as<SubroutineSymbol>()))
        continue;
      // The frontend infers no clock for a `$inferred_clock` formal default
      // of a static checker instance, but 16.15.7 and 17.4 take the clock
      // that flows to the instantiation (its default clocking). The owned
      // checker lowering resolves it there and reports a located error when
      // none applies.
      if (diagnostic.code == diag::NoInferredClock && inStaticCheckerBody(diagnostic.symbol))
        continue;
      // The unused-declaration check does not count a checker instantiated
      // with the module instantiation syntax (IEEE 1800-2009 17.3) as a use.
      if ((diagnostic.code == diag::UnusedAssertionDecl ||
           diagnostic.code == diag::UnusedPackageAssertionDecl) &&
          diagnostic.symbol && diagnostic.symbol->kind == SymbolKind::Checker) {
        if (!instantiatedCheckers)
          instantiatedCheckers = collectInstantiatedCheckers(root);
        if (instantiatedCheckers->contains(diagnostic.symbol))
          continue;
      }
      analysisClient->issue(engine, diagnostic);
    }
    output->flags |= LLG_SLANG_SNAPSHOT_ANALYSIS_RAN;
  }

  ProfileStage semanticStage("wrapper.semantic");
  if (!capture.declarationOnly) {
    SemanticCapture semanticCapture(capture);
    root.visit(semanticCapture);
    for (const Symbol* definition : compilation.getDefinitions())
      definition->visit(semanticCapture);
    semanticCapture.captureShadowedDefinitions(compilation, root);
  }
  semanticStage.finish();
  ProfileStage finalizeStage("wrapper.finalize");
  capture.markOverriddenParameters();
  capture.finalizeSourceIdentities();
  capture.finalizeSemanticEdges();

  finalizeStage.finish();
  ProfileStage lexicalStage("wrapper.lexical");
  LexicalCapture lexicalCapture(capture);
  for (const auto& tree : compilation.getSyntaxTrees())
    tree->root().visit(lexicalCapture);
  lexicalCapture.bindSemanticTokens();
  lexicalCapture.finishProtectedEnvelopes();
  lexicalCapture.finishFsmPragmas(root);
  if (edition.snapshotFlag == LLG_SLANG_SNAPSHOT_EDITION_VERILOG_2001) {
    Verilog2001SyntaxProfile profile(capture);
    for (const auto& tree : compilation.getSyntaxTrees())
      profile.walk(tree->root());
  }

  lexicalStage.finish();
  return output;
}

// Records delivered to the sink per callback. Bounds the borrowed batch, not
// memory: each table is released as a whole after delivery, semantic nodes
// chunk by chunk.
constexpr size_t kStreamBatchRecords = 4096;

// Thrown when the sink asks to stop; the receiver owns the actual error.
class SinkAborted final : public std::exception {
public:
  const char* what() const noexcept override {
    return "capture sink rejected the stream";
  }
};

bool sinkComplete(const LlgSlangSink& sink) {
  return sink.begin && sink.files && sink.related_diagnostics &&
         sink.diagnostics && sink.value_words && sink.constants &&
         sink.type_ranges && sink.type_members && sink.types &&
         sink.parameters && sink.instances && sink.semantic_edges &&
         sink.semantic_nodes && sink.udp_rows && sink.udp_tables &&
         sink.lexical_tokens && sink.source_libraries &&
         sink.line_directives && sink.edition_findings &&
         sink.protected_envelopes && sink.fsm_pragmas && sink.end;
}

void checkSink(uint32_t status) {
  if (status != LLG_SLANG_SINK_CONTINUE)
    throw SinkAborted();
}

template<typename T, typename Callback>
void streamTable(const LlgSlangSink& sink, Callback callback, std::vector<T>& table) {
  for (size_t start = 0; start < table.size(); start += kStreamBatchRecords) {
    const size_t count = std::min(kStreamBatchRecords, table.size() - start);
    checkSink(callback(sink.context, table.data() + start, static_cast<uint64_t>(count)));
  }
  std::vector<T>().swap(table);
}

// Deliver a finished capture and release it table by table. The Slang
// compilation is already destroyed; only the interned strings stay alive
// until the last record that may borrow them has been delivered.
void streamCapture(CaptureOutput& output, const LlgSlangSink& sink) {
  ProfileStage stage("wrapper.stream");
  const LlgSlangStreamHeader header{
      LLG_SLANG_ABI_VERSION,
      output.flags,
      output.files.size(),
      output.related.size(),
      output.diagnostics.size(),
      output.value_words.size(),
      output.constants.size(),
      output.type_ranges.size(),
      output.type_members.size(),
      output.types.size(),
      output.parameters.size(),
      output.instances.size(),
      output.semantic_edges.size(),
      output.semantic_nodes.size(),
      output.udp_rows.size(),
      output.udp_tables.size(),
      output.lexical_tokens.size(),
      output.source_libraries.size(),
      output.line_directives.size(),
      output.edition_findings.size(),
      output.protected_envelopes.size(),
      output.fsm_pragmas.size(),
  };
  checkSink(sink.begin(sink.context, &header));
  streamTable(sink, sink.files, output.files);
  streamTable(sink, sink.related_diagnostics, output.related);
  streamTable(sink, sink.diagnostics, output.diagnostics);
  streamTable(sink, sink.value_words, output.value_words);
  streamTable(sink, sink.constants, output.constants);
  streamTable(sink, sink.type_ranges, output.type_ranges);
  streamTable(sink, sink.type_members, output.type_members);
  streamTable(sink, sink.types, output.types);
  streamTable(sink, sink.parameters, output.parameters);
  streamTable(sink, sink.instances, output.instances);
  streamTable(sink, sink.semantic_edges, output.semantic_edges);
  for (size_t chunk = 0; chunk < output.semantic_nodes.chunkCount(); chunk++) {
    checkSink(sink.semantic_nodes(sink.context, output.semantic_nodes.chunkData(chunk),
                                  output.semantic_nodes.chunkSize(chunk)));
    output.semantic_nodes.releaseChunk(chunk);
  }
  streamTable(sink, sink.udp_rows, output.udp_rows);
  streamTable(sink, sink.udp_tables, output.udp_tables);
  streamTable(sink, sink.lexical_tokens, output.lexical_tokens);
  streamTable(sink, sink.source_libraries, output.source_libraries);
  streamTable(sink, sink.line_directives, output.line_directives);
  streamTable(sink, sink.edition_findings, output.edition_findings);
  streamTable(sink, sink.protected_envelopes, output.protected_envelopes);
  streamTable(sink, sink.fsm_pragmas, output.fsm_pragmas);
  checkSink(sink.end(sink.context));
}

// Definition names a compilation unit declares at its outermost level, the
// scope whose definitions other units can instantiate, import or name.
// Nested modules and classes inside other declarations are local.
void unitDeclaredNames(const syntax::SyntaxNode& root,
                       std::unordered_set<std::string_view>& names) {
  if (root.kind != syntax::SyntaxKind::CompilationUnit)
    return;
  for (const syntax::MemberSyntax* member :
       root.as<syntax::CompilationUnitSyntax>().members) {
    parsing::Token name;
    switch (member->kind) {
      case syntax::SyntaxKind::ModuleDeclaration:
      case syntax::SyntaxKind::InterfaceDeclaration:
      case syntax::SyntaxKind::ProgramDeclaration:
      case syntax::SyntaxKind::PackageDeclaration:
        name = member->as<syntax::ModuleDeclarationSyntax>().header->name;
        break;
      case syntax::SyntaxKind::UdpDeclaration:
        name = member->as<syntax::UdpDeclarationSyntax>().name;
        break;
      case syntax::SyntaxKind::CheckerDeclaration:
        name = member->as<syntax::CheckerDeclarationSyntax>().name;
        break;
      case syntax::SyntaxKind::ClassDeclaration:
        name = member->as<syntax::ClassDeclarationSyntax>().name;
        break;
      default:
        continue;
    }
    if (!name.valueText().empty())
      names.emplace(name.valueText());
  }
}

// Report one parsed tree's declared and referenced definition names to the
// sink under `sourceIndex`. Referenced names exclude every definition the
// tree declares itself, including nested modules and classes (Slang's
// ParserMetadata declared set), and the built-in `std` package.
void reportTreeMetadata(const syntax::SyntaxTree& tree, uint64_t sourceIndex,
                        const LlgSlangMetadataSink& sink) {
  std::unordered_set<std::string_view> declared;
  unitDeclaredNames(tree.root(), declared);
  std::unordered_set<std::string_view> local = declared;
  const parsing::ParserMetadata& metadata = tree.getMetadata();
  metadata.visitDeclaredSymbols([&](std::string_view name) { local.emplace(name); });
  std::vector<std::string_view> referenced;
  std::unordered_set<std::string_view> seen;
  metadata.visitReferencedSymbols([&](std::string_view name) {
    if (!name.empty() && name != "std" && !local.contains(name) &&
        seen.emplace(name).second)
      referenced.push_back(name);
  });
  std::vector<std::string_view> declaredNames(declared.begin(), declared.end());
  // Deterministic delivery order independent of hash iteration.
  std::sort(declaredNames.begin(), declaredNames.end());
  std::sort(referenced.begin(), referenced.end());
  auto deliver = [&](uint32_t role, std::string_view name) {
    checkSink(sink.name(sink.context, sourceIndex, role,
                        LlgSlangString{reinterpret_cast<const uint8_t*>(name.data()),
                                       static_cast<uint64_t>(name.size())}));
  };
  for (std::string_view name : declaredNames)
    deliver(LLG_SLANG_METADATA_DECLARED, name);
  for (std::string_view name : referenced)
    deliver(LLG_SLANG_METADATA_REFERENCED, name);
}

// Parse-only metadata request: preprocess and parse each compilation-unit
// buffer with the compile's edition and defines (cache-only include lookup,
// diagnostics discarded) and stream its definition names. Nothing is
// elaborated and no native state outlives the call.
void metadataImpl(const LlgSlangMetadataRequest& request,
                  const LlgSlangMetadataSink& sink) {
  if (request.abi_version != LLG_SLANG_ABI_VERSION)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unsupported Slang ABI version");
  constexpr uint32_t knownFlags = LLG_SLANG_COMPILE_EDITION_VERILOG_2001 |
                                  LLG_SLANG_COMPILE_EDITION_SYSTEMVERILOG_2009 |
                                  LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS;
  if ((request.flags & ~knownFlags) != 0)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "unknown metadata request flags");
  const EditionPolicy edition = editionPolicy(request.flags);
  if (request.source_count != 0 && request.sources == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT, "sources has a null pointer");
  if (request.define_count != 0 && request.defines == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT, "defines has a null pointer");
  if (request.include_dir_count != 0 && request.include_dirs == nullptr)
    throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                        "include_dirs has a null pointer");
  if (request.source_count > kHardMaxSources)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "source count limit exceeded");
  if (request.define_count > kHardMaxDefines)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED, "define count limit exceeded");
  if (request.include_dir_count > kHardMaxIncludeDirs)
    throw BridgeFailure(LLG_SLANG_STATUS_LIMIT_EXCEEDED,
                        "include directory count limit exceeded");
  const uint64_t maxSourceBytes = effectiveLimit(
      request.max_source_bytes, kDefaultMaxSourceBytes, kHardMaxSourceBytes);

  uint64_t sourceBytes = 0;
  uint64_t configBytes = 0;
  std::vector<std::string> sourcePaths;
  sourcePaths.reserve(static_cast<size_t>(request.source_count));
  std::unordered_set<std::string> uniquePaths;
  uniquePaths.reserve(static_cast<size_t>(request.source_count));
  std::vector<std::string_view> sourceTexts;
  sourceTexts.reserve(static_cast<size_t>(request.source_count));
  for (uint64_t i = 0; i < request.source_count; i++) {
    const auto& input = request.sources[i];
    if ((input.flags & ~LLG_SLANG_SOURCE_COMPILATION_UNIT) != 0 || input.reserved != 0)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "unknown metadata source flags or nonzero reserved field");
    const std::string_view name = checkedView(input.name, "source name");
    const std::string_view text = checkedView(input.text, "source text");
    if (name.empty() || name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "source names must be nonempty and contain no NUL bytes");
    addChecked(sourceBytes, name.size(), maxSourceBytes, "source byte");
    addChecked(sourceBytes, text.size(), maxSourceBytes, "source byte");
    std::string normalized = std::filesystem::path(name).lexically_normal().generic_string();
    if (!uniquePaths.emplace(normalized).second)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "source names must be unique after lexical normalization");
    sourcePaths.push_back(std::move(normalized));
    sourceTexts.push_back(text);
  }

  std::vector<std::string> predefines;
  predefines.reserve(static_cast<size_t>(request.define_count));
  for (uint64_t i = 0; i < request.define_count; i++) {
    const auto& input = request.defines[i];
    if (input.has_value > 1 || input.reserved != 0)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT, "invalid define flags");
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
  std::vector<std::filesystem::path> includeDirs;
  includeDirs.reserve(static_cast<size_t>(request.include_dir_count));
  for (uint64_t i = 0; i < request.include_dir_count; i++) {
    const std::string_view name = checkedView(request.include_dirs[i], "include directory");
    if (name.find('\0') != std::string_view::npos)
      throw BridgeFailure(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                          "include directories must contain no NUL bytes");
    addChecked(configBytes, name.size(), kHardMaxConfigBytes, "configuration byte");
    includeDirs.emplace_back(std::filesystem::path(name).lexically_normal());
  }

  SourceManager sourceManager;
  sourceManager.setDisableProximatePaths(true);
  sourceManager.setCacheOnlyReads(true);
  std::vector<SourceBuffer> buffers;
  buffers.reserve(static_cast<size_t>(request.source_count));
  for (uint64_t i = 0; i < request.source_count; i++)
    buffers.push_back(sourceManager.assignText(sourcePaths[static_cast<size_t>(i)],
                                               sourceTexts[static_cast<size_t>(i)]));

  parsing::PreprocessorOptions preprocessorOptions;
  preprocessorOptions.languageVersion = edition.languageVersion;
  preprocessorOptions.predefines = std::move(predefines);
  preprocessorOptions.additionalIncludePaths = std::move(includeDirs);
  preprocessorOptions.keywordMapping.reserve(sourcePaths.size());
  for (const std::string& path : sourcePaths)
    preprocessorOptions.keywordMapping.emplace_back(path, edition.keywordVersion);
  parsing::LexerOptions lexerOptions;
  lexerOptions.languageVersion = edition.languageVersion;
  // Verilog-XL `protected/`endprotected envelopes are lexed (the encoded text
  // is skipped like a pragma protect data block) instead of being reported as
  // unknown directives; `protect/`endprotect become ignored directives.
  // The metadata parse lexes exactly as the compile does.
  lexerOptions.enableLegacyProtect = true;
  parsing::ParserOptions parserOptions;
  parserOptions.languageVersion = edition.languageVersion;
  Bag parseOptions;
  parseOptions.set(std::move(preprocessorOptions));
  parseOptions.set(std::move(lexerOptions));
  parseOptions.set(std::move(parserOptions));

  if ((request.flags & LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS) != 0) {
    std::vector<SourceBuffer> unitBuffers;
    uint64_t firstUnit = 0;
    for (uint64_t i = 0; i < request.source_count; i++) {
      if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) == 0)
        continue;
      if (unitBuffers.empty())
        firstUnit = i;
      unitBuffers.push_back(buffers[static_cast<size_t>(i)]);
    }
    if (!unitBuffers.empty()) {
      auto tree = syntax::SyntaxTree::fromBuffers(unitBuffers, sourceManager, parseOptions);
      reportTreeMetadata(*tree, firstUnit, sink);
    }
    return;
  }
  for (uint64_t i = 0; i < request.source_count; i++) {
    if ((request.sources[i].flags & LLG_SLANG_SOURCE_COMPILATION_UNIT) == 0)
      continue;
    // Each tree (and the names it borrows) is released before the next parse.
    auto tree = syntax::SyntaxTree::fromBuffer(buffers[static_cast<size_t>(i)],
                                               sourceManager, parseOptions);
    reportTreeMetadata(*tree, i, sink);
  }
}

} // namespace

extern "C" uint32_t llg_slang_compile(const LlgSlangCompileRequest* request,
                                       const LlgSlangSink* sink,
                                       LlgSlangError** out_error) {
  if (out_error)
    *out_error = nullptr;
  if (!request || !sink || !out_error || !sinkComplete(*sink)) {
    if (out_error)
      *out_error = makeError(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                             "request, sink callbacks and error output are required");
    return LLG_SLANG_STATUS_INVALID_ARGUMENT;
  }

  try {
    auto output = compileImpl(*request);
    // The compilation, syntax trees and capture indexes are destroyed by
    // now. glibc keeps their freed pages resident inside its heap; return
    // them to the system before the receiver builds its owned copy.
    llg::wrapper::platform::releaseFreedHeapPages();
    streamCapture(*output, *sink);
    output.reset();
    // Delivered node chunks and the interned strings were freed while the
    // receiver allocated its copy between them; return the free pages
    // inside the heap before the caller's import allocates more.
    llg::wrapper::platform::releaseFreedHeapPages();
    return LLG_SLANG_STATUS_OK;
  }
  catch (const SinkAborted& error) {
    *out_error = makeError(LLG_SLANG_STATUS_SINK_ABORTED, error.what());
    return LLG_SLANG_STATUS_SINK_ABORTED;
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

extern "C" uint32_t llg_slang_parse_metadata(const LlgSlangMetadataRequest* request,
                                             const LlgSlangMetadataSink* sink,
                                             LlgSlangError** out_error) {
  if (out_error)
    *out_error = nullptr;
  if (!request || !sink || !sink->name || !out_error) {
    if (out_error)
      *out_error = makeError(LLG_SLANG_STATUS_INVALID_ARGUMENT,
                             "request, sink callback and error output are required");
    return LLG_SLANG_STATUS_INVALID_ARGUMENT;
  }
  try {
    metadataImpl(*request, *sink);
    return LLG_SLANG_STATUS_OK;
  }
  catch (const SinkAborted& error) {
    *out_error = makeError(LLG_SLANG_STATUS_SINK_ABORTED, error.what());
    return LLG_SLANG_STATUS_SINK_ABORTED;
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

extern "C" void llg_slang_error_destroy(LlgSlangError* error) {
  if (error && !error->is_static)
    delete error;
}
