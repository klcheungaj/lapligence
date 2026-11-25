# Vendored source patches

Root `build.rs` applies these patches with a portable Rust preflight, retaining
upstream-base vendor gitlinks. Clean and fully applied trees are accepted;
files an earlier committed patch set applied are restored first
([stale build output](#stale-build-output)); other mixed/mismatched inputs fail,
including edits outside diff hunks.

## Content and filesystem validation

Each patch directory has complete clean/applied `files.sha256` digests and a
required retired-target manifest. Slang's authenticated retired set includes the
old `SVInt.cpp` target. Missing, unexpectedly empty, extra or mismatched manifest
content fails with or without Git.

Digests use canonical LF. `.gitattributes` tracks patches/manifests as LF;
rendering preserves vendor CRLF where present. Authenticate the rendered applied
digest before writing.

`symlink_metadata` plus link-free ancestor opens reject symlinks, Windows reparse
points and outside resolutions. Repository/parent capabilities remain attached
through staging, permission changes, cleanup and same-directory rename using safe
`cap-std`/`cap-fs-ext` APIs. Linux stages anonymous `O_TMPFILE` and publishes via
handle-relative `linkat`; Windows uses an exclusive handle; other targets retain
identity/link-count checks. Flush file and parent, rejecting changed capabilities
or inserted hardlinks before accepting replacement.

With usable Git metadata, require canonical vendor `rev-parse --show-toplevel`,
pinned HEAD, no out-of-set tracked changes and no untracked source/build inputs.
No-Git/copied archives, including those inside unrelated checkouts, use exact
manifests without Git or safe.directory setup. Slang lists sources explicitly;
generated products stay outside vendors. These fixed inputs bound archive
admission.

## Stale build output

After a branch switch or pull changes a manifest, the vendor tree may still
hold files the build applied from an older patch set. When a patch or retired
target is neither clean nor currently applied and Git metadata is usable (the
project root is exactly a Git top level; Slang also passes the checks above,
counting retired targets as admitted changes), the preparer searches the
applied digests of the last `STALE_MANIFEST_HISTORY_LIMIT` committed revisions
of that `files.sha256` (`git log` from `HEAD`) for each such path. Only if every
such file is identified does it restore every non-clean target, current
applied ones included, to the pinned base blob (vendor `HEAD` for Slang,
superproject `HEAD` for libfst), authenticated against the current base digest
and written through the staging path above, warn with the files and matched
commits, and apply normally. Otherwise, or without Git, nothing changes and the
error names the files and the manual fix, which discards all local vendor
edits: `git -C vendor/slang checkout -- .` or `git restore -- vendor/libfst`.

## Slang

- Base: `7ddf4059f79eff508dd486eb42fd650cdf320d52`
- Source: `dc2161b6e7f87a8057e9498a7c983a2270d43c12`

| Patch | Purpose |
| --- | --- |
| `slang/slang-cache-only-source-reads.patch` | Cache-only admitted-buffer reads and lexical path normalization. |
| `slang/slang-ref-port-binding.patch` | Packed module-ref lvalue binding; subroutine refs retain separate rules. `inout uwire` formals are admitted (SV 6.6.2, 23.3.3.6–23.3.3.7); the simulator checks the collapsed net's drivers. It also owns the multi-port connection split (a concatenated port expression, SV 23.2.2.1): an instance array gives each instance its slice of a packed actual first (SV 23.3.3.5), and an actual narrower than the multi-port connects the last component to the remaining bits only, leaving higher components unconnected (upstream selected past the actual's end, which named the whole actual). |
| `slang/slang-multiclock-segment-empty-match.patch` | Multiclocked concatenation checks the whole maximal singly clocked subsequence before a clock change for an empty match (SV 16.14.1), not only its last element, so `a ##1 b[*0:1] ##1 @(c2) d` is accepted while `a[*0:1] ##1 @(c2) d` is still rejected. Upstream analysis error, not an edition policy. |
| `slang/slang-package-wildcard-export.patch` | Lazy finite wildcard re-exports, with ambiguous names diagnosed. |
| `slang/slang-conditional-z-merge.patch` | Selected packed conditional policy: definite true selects one arm despite other X/Z predicate bits; unpacked constants merge leaf by leaf, recursing through nested arrays and structures (decision R05-D1): matching leaves are kept and the others default by type, not initializer. Local edition policy, not an upstream erratum. Because patches cannot overlap, it also carries the operator-overload hooks in `OperatorExpressions.cpp` (below), including the relational-operand expected type. |
| `slang/slang-output-port-runtime-select.patch` | Output ports connected to variables (implied continuous assignments, SV 23.3.3.2) and continuous assignments to variables (SV 10.3, A.8.5 `variable_lvalue`) admit runtime selects; the written variable is the select's root. Net, inout and gate-terminal lvalues keep constant selects. Because it owns `SelectExpressions.cpp`, it also binds the function form of `rand_mode` on a `rand` class handle property, or on one element of it, to the built-in query of that variable (SV 18.8): upstream found only the class's own `rand_mode` task there and asked for its argument. A handle that is not a random variable keeps that error. It also defines `ASTFlags::ForceTarget`, under which an element select of a static unpacked array variable may take a runtime index in a force or release target (SV 10.6.2, 6.4); see `slang-static-array-element-force.patch`. |
| `slang/slang-static-array-element-force.patch` | In SystemVerilog (1800-2005 and later) a force or release target may be an element of a static unpacked array variable, with a constant or runtime index, or a member of an unpacked-structure element of one: an element of an unpacked array is a singular variable (SV 6.4, 10.6.2). A bit- or part-select of an element stays a select of a variable and is rejected; dynamic-array and queue elements keep their dynamic-context error (SV 6.21); IEEE 1364 keeps the memory-word error (V2001 9.3.2). The flag that admits the runtime index lives in `slang-output-port-runtime-select.patch`, which owns `ASTContext.h` and `SelectExpressions.cpp`. Upstream rejected every element of an unpacked array. |
| `slang/slang-operator-overload.patch` | Operator overload declarations (SV 11.11, A.2.8): `OverloadDeclaration` syntax in module, package, compilation-unit, generate and block items, an unnamed `OperatorOverload` symbol, and resolution only where a built-in unary, binary, increment, compound, assignment or cast operation is illegal. A resolved use becomes an ordinary `CallExpression` to the function found from the use's scope. In statement position (an expression statement or a `for` step, whose value is discarded) an increment or compound assignment with a side-effect-free target becomes `A = f(A, ...)`; otherwise it keeps its compound operator and Slang's lvalue placeholder as the call's first argument, so the target is bound once, and `AssignmentExpression::isOverloadedPostfix` marks a postfix value. Relational operands whose overload is ambiguous alone take the other operand's type (`Compilation::OverloadAmbiguityProbe`). A wildcard-imported package's overloads are candidates after the scope's own (project ruling in `docs/sim_data_semantics.md`). A prototype whose operator is already legal for its formal types, with a built-in result assignable to its result type, is rejected at the declaration (`OverloadBuiltinLegal`, SIM-021). Because patches cannot overlap, it also carries an error-recovery fix in `Statement.cpp`: a labeled statement, named block, declaring or labeled `for`, `foreach` or `randsequence` finds its pre-created block even when an earlier statement failed before binding its children (a bad `for` header), instead of dereferencing a missing block. Its `ElabVisitors.h` hunk keeps an instance body that executes an elaboration system task out of the instance cache, so every call reports (SV 20.10). Its `AnalysisScopeVisitor.h` hunk also stops the shadowing check from reporting a transparent member (a specify-block specparam, an anonymous-program member) as shadowing its own hoisted copy. |
| `slang/slang-net-alias-members-uwire-inout.patch` | Net aliases may name constant member selects of structure nets (SV 10.11, A.8.3/A.8.5). A uwire net may connect to a module inout port, also inside a concatenation, and neither that connection nor an `inout uwire` formal's port counts as a uwire driver (SV 6.6.2, 23.3.3.6–23.3.3.7); the simulator checks the collapsed net's drivers. Pass-switch terminals stay rejected. Gate and UDP output terminals (`net_lvalue`, A.3.3) keep constant selects on variables. Because it owns `MemberSymbols.cpp`, it also creates a randsequence rule's implicit result variables (SV 18.17.7) in order of first use instead of hash-map order, which followed symbol addresses and made generated models differ between runs, and names the scope of the call in an elaboration system task's message (SV 20.10: "The hierarchical name of the scope in which the elaboration system task call is made"). |
| `slang/slang-specialization-order.patch` | `GenericClassDefSymbol` keeps its specializations in order of creation as well as in their address-keyed map, and iterates them in that order. Upstream iterated the map, so the elaboration order, and with it the order and number of the diagnostics that specializations repeat with different arguments, changed between identical runs. `DriverTracker` propagates the drivers of `ref` ports and modport ports in the order it noted them (its files are in `slang-net-alias-members-uwire-inout.patch`), the constraint analysis looks for cycles in the order the constraints name their variables, and the note of `InvalidPulseStyle` or `VirtualIfaceDefparam` (`Expression.cpp`, in `slang-operator-overload.patch`) names the first path or defparam in the source; upstream followed address-keyed maps, so a run could report another of several such diagnostics than the run before. |
| `slang/slang-invalid-source-recovery.patch` | A sequence or repetition range whose minimum failed to evaluate is not compared with its maximum (upstream read an empty optional). Patches that own their files carry the other fixes for rejected sources: `slang-typename-resolved.patch` (printing an enumeration whose value failed to evaluate), `slang-operator-overload.patch` (a checker output formal without an actual; a `foreach` loop variable beyond the last iterable dimension is `TooManyForeachVars` for any element type), and `slang-output-port-runtime-select.patch` (a member of a constant null class handle is no constant). Upstream threw, dereferenced null or accepted them. |
| `slang/slang-type-parameter-reference.patch` | A lookup that finds a type parameter notes the reference on its alias, which carries the declaration syntax. Upstream noted it on the parameter symbol, which has no syntax when an instance overrides the parameter, so a type parameter overridden everywhere never counted as used. |
| `slang/slang-typename-resolved.patch` | `TypePrinter::typenameText`, the text of `$typename` (SV 20.6.1), with the printing option `resolvedTypeNames`: a structure, union or enumeration is named by the typedef that declares it, also through another typedef or a type parameter, and a user-defined class or covergroup with its scope. The constant evaluator's `$typename` uses it; the bridge captures it per call. Diagnostics print types as before. |
| `slang/slang-macro-file-names.patch` | `PreprocessorOptions::macroFileNames` names what the `` `__FILE__`` intrinsic reports for a file, keyed by the source manager's full path: the path by which the file was opened (IEEE 1800-2009 22.13), which the bridge passes per buffer. A `` `line`` rename still wins; an unlisted file keeps `SourceManager::getFileName`. Diagnostics and positions are unchanged. |
| `slang/slang-top-type-parameter-overrides.patch` | Command-line overrides (`Compilation` param overrides) also apply to top-level type parameters: the override expression, a data type the bridge spells as `type(...)`, becomes the type parameter's assignment unless an instance or configuration assignment exists, and a type parameter without a default is a valid top when overridden. Upstream ignores type parameters here. |
| `slang/slang-parse-tree-depth.patch` | The parser's depth guard (`ParserOptions::maxRecursionDepth`, `ParseTreeTooDeep`) also counts the nodes a loop puts above one another: operators of one precedence, members, selects, dimensions and event lists. Upstream guarded its own recursion only, so a left-associative chain of any length was accepted and every later walk of the tree recursed over it. Guards are added where upstream recursed without one: generate and class members, constraint items, coverage and bins-select expressions, randsequence productions, `not` and `nexttime` of a property, a repeated event control. The exception carries the text of the diagnostic, which is all that a caller parsing one expression (a parameter override) sees. `slang-operator-overload.patch` carries the guard itself (`Parser.h`) and the hunks in `Parser_members.cpp` and `Parser_statements.cpp`, and `slang-net-alias-members-uwire-inout.patch` the limit on a chain of `let`, sequence and property instances in `MiscExpressions.cpp` (`maxInstanceDepth`, `MaxInstanceDepthExceeded`): each expansion binds inside the one that names it. |
| `slang/slang-endless-specialization.patch` | A class that names itself with ever new parameter values, directly, through a base class, a typedef, a subroutine, another class or a virtual interface, is an error where it does (`RecursiveClassSpecialization`) once `maxRecursiveClassSpecialization` of its specializations enclose one another; the limit is the bridge's nesting limit and also replaces upstream's fixed 8 for specializations nested in parameter assignments. A virtual interface type whose interface names itself is limited like an instance (`MaxInstanceDepthExceeded`). Upstream elaborated all of them until memory ran out. Elaboration visits the specializations and virtual interfaces that a visit adds before their siblings, so one with several branches per level meets its limit before they multiply, and after any depth limit (`Compilation::noteHierarchyTooDeep`) nothing that nests in itself is created any more, so later walks of the AST end too. A refused checker instance has no name and no longer hides its checker. `slang-specialization-order.patch` (`ClassSymbols`), `slang-operator-overload.patch` (`Compilation.h`, `ElabVisitors.h`), `slang-net-alias-members-uwire-inout.patch` (`InstanceSymbols.cpp`) and `slang-block-parameter-defparam.patch` (`InstanceSymbols.h`, `DefParamResolver.cpp`) carry the hunks in the files they own. |
| `slang/slang-block-parameter-defparam.patch` | A `parameter` declared in a block, task or function is not local (SV 6.20.4 makes `parameter` a synonym for `localparam` only in a generate block, package, class body and compilation unit) and a defparam redefines it (SV 23.10.2). Overrides are kept per instance and generate block, so generate iterations that share the declaration differ. A block parameter that no instance holds (a class method, a package function) stays rejected. The value of a defparam also gives a module or block parameter declared without a type and a range its type and range, and a string literal makes it an implicit string (SV 6.20.2, 23.10); a change of that size alone repeats the defparam resolution. Upstream made every block parameter local and kept the type of the default value. `slang-operator-overload.patch`, which owns `Compilation.h`, records the string literal in `HierarchyOverrideNode`. `slang-net-alias-members-uwire-inout.patch`, which owns `InstanceSymbols.cpp`, lets `findParentOverrideNode` pass procedural scopes and gives it external linkage. |
| `slang/slang-randomize-inherited-property.patch` | `randomize(list)` accepts properties a base class declares (SV 18.11); upstream accepted only the object's own class. |
| `slang/slang-integral-string-arguments.patch` | The string arguments of `$fopen`, `$test$plusargs`, `$timeformat`, `$system`, `$dumpfile` and `$dumpports*` may be integral (SV 21.3.1, 21.7: "an integral data type containing a character string"; the text does not restrict the `$timeformat` suffix); the value converts as by a cast to `string`. Upstream rejected them; the coverage database tasks keep the string type (decision S26-D23). |
| `slang/slang-sdf-annotate-arguments.patch` | `$sdf_annotate` accepts empty arguments after the file name (SV Syntax 32-1, V Syntax 17-8: `$sdf_annotate("f.sdf", , , , "MAXIMUM")`), and each argument other than the scope, not only the first, must be string-like. Upstream rejected every empty argument and checked the first argument's type once per argument. Because it owns `SystemTasks.cpp`, it also admits a statement block, task or function as a `$dumpvars` scope argument. It also lets `$monitor`, `$strobe` and the file variants name class properties and `this` (SV 13.3.2 bars only variables of automatic subroutines; a property lives in its object). |

## Updating patches

Transition from the verified prior complete applied state or documented clean
base, then run the preparer; committing the new manifest lets a later build
recognise the prior applied files (above). Never mix old applied files with new
manifests, relax full-file authentication, admit arbitrary intermediates, or
discard unrelated vendor edits to satisfy preflight.

## libfst

`vendor/libfst` is a plain directory of pristine upstream files tracked by this
repository (no submodule). The same preparer that patches Slang applies
`libfst/*.patch` in place there, with the same acceptance rules: a clean or a
fully applied tree, partial or mismatched content rejected with the offending
files named, nothing rewritten when already applied. Digests come from
`libfst/files.sha256`; `retired-files.sha256` is empty. The runtime
(`waveform_sources()`) and the standalone CMake probes read `vendor/libfst`
directly; the probes only verify the applied digests
(`tests/runtime_value_storage/libfst.cmake`) and ask for `cargo build` otherwise.
CI jobs without a Rust build (`dynamic-owners`, the Linux `test` archive runs)
apply the patch first with `git apply --whitespace=nowarn
--directory=vendor/libfst patches/libfst/libfst-local-changes.patch` from the
repository root, which yields the identical files.

Because the files are tracked, a built tree shows them as modified in
`git status`. **Never commit or stage the applied state.** The
`vendor_patches` test `committed_libfst_blobs_are_pristine` fails when HEAD or
the index holds anything but the pristine digests, so CI and any commit-time
test run catch it. Undo a build with
`git restore --staged --worktree -- vendor/libfst`; the next build re-applies
the patch. Local reproducible-run scripts accept the authenticated applied
state (`scripts/run-regression.sh`). A submodule would give a `dirty` marker
like Slang's instead of tracked modifications, at the cost of a separate
repository for nine files.

- Source: GTKWave libfst as shipped with Verilator 5.032 (`include/gtkwave/`)

| Patch | Purpose |
| --- | --- |
| `libfst/libfst-local-changes.patch` | Bounded writer buffering: the 128 MiB block and 2 GiB growth ceiling become 1 MiB with a 256 KiB increment. `_GNU_SOURCE` on non-Windows hosts so strict `-std=c11` builds declare the POSIX calls libfst uses. MSVC: `fst_config.h` leaves `HAVE_ALLOCA_H`/`HAVE_FSEEKO` undefined, `fstapi.c` takes its MinGW Win32 paths (`FST_WIN32_API`: file mapping, temporary files, `_fseeki64`, unbuffered reads), and the varint readers size their check buffers with enum constants instead of `const int` arrays (VLAs, rejected by MSVC). Win32 `tmpfile_open` opens the file `GetTempFileName` created in place: deleting it first released the reserved name to other writers in `%TEMP%`. `fstWriterCreate` names each failed step on stderr with its path and OS error (`_doserrno` too under `FST_WIN32_API`) and prints nothing on success. |
