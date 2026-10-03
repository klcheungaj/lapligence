# Fixed lvalue and reference-port projections

Public CLI fixtures execute in both HDL optimizer modes. `nested_ports`,
`array_record`, `wide_port`, `ref_scan_selected` and `selector_capture` also execute after snapshot and Db destruction
with native O0/O3 in both modes. Outputs are hand-derived from physical bit
positions and sequential selector counts, independently of the simulator.

| Fixture | IEEE 1800-2009 contract and oracle |
| --- | --- |
| `ref_port_select` | Adopted FND-002 witness, §§23.2.2.3/23.3.3: writing 9 into the low nibble of the high byte produces 0900. |
| `nested_ports` | §§7.4.6/11.5/23.2.2.3: descending and ascending ref chains preserve immediate widths. Descending leaf writes physical bits 5:4 as 10; ascending writes bits 3:2 as 11. Partially invalid writes add low bits 11 / 10, yielding 23 / 0e. Missing read bits are X. X/Z and huge unsigned selectors write nothing. |
| `bit_port` | §§11.5/23.2.2.3: a one-bit ref at root bit 2 can write only that bit; neighboring inner indices remain invalid even when root neighbors exist. |
| `array_record` | §§7.4.6/11.5/23.2.2.3: selecting ascending member positions 2:5, then 1:2, writes physical bit 4. Record pad F and the other row ABC remain unchanged. |
| `selector_capture` | §§10.4.1/10.4.2/11.4.2/13.5: counts advance exactly once per destination. NBA captures slot zero before it changes to six; timed output captures slot two before the callee changes it to four. Prefix/postfix return new/old values. Overlapping NBA captures low nibble B before it is cleared, then writes B into bits 5:2, giving 2C. |
| `wide_port` | §§7.4.6/11.5/23.2.2.3: writes cross a 64-bit limb while the 65-bit ref clips upper/lower partial selects. Final root bits 80:78 are 111 and bits 17:16 are 11. |
| `ref_scan_selected` | §§11.5/13.5.2/21.3.4.3: scanning A into bits 5:2 of 81 produces A9; a partial scan sets bits 1:0 to 11, producing AB. An X selector writes nothing and a failed decimal conversion leaves AB unchanged. Selector counts are 1/2/3. A subroutine ref forwarded from a selected module ref preserves the root nibble neighbors. |
| `ref_scan` | Adopted FND-002 witness, §§21.3.4.3/13.5.2: scanning 7 through a fixed-array ref formal assigns one destination and returns one conversion. |

Adopted witnesses use quiet `$finish(0)`; selector/write semantics are unchanged.

Adopted FND-002 negatives: `neg_ref_temporary` and `neg_ref_packed_select`
(§13.5.2), `neg_mutation_temporary` (§11.4.2). `neg_ref_part` separately
excludes a packed part-select subroutine actual. These do not restrict legal
module-ref packed projections.

`runtime_ref_port` is an adopted FND-002 unresolved characterization. It asserts
the project's explicit unsupported runtime-binding diagnostic, **not** language
illegality or a guessed dynamic binding oracle. Static counterparts execute.
No runtime operations, allocation mechanisms or suspension points were added;
the timed copy-out case exercises existing retained target ownership.
