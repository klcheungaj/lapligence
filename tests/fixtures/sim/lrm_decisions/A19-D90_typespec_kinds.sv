// Decision A19-D90: chandle, void and enum base typespecs, vpiPacked of a
// packed tagged union, and no interface typespec.
// - A chandle has a vpiChandleTypespec; a typedef of chandle names it and
//   leads to the unnamed built-in chandle typespec by vpiTypedefAlias.
// - The vpiBaseTypespec of an enum whose base type is a typedef is that
//   typedef's typespec, named after it; a built-in base has no name.
// - A void member of a tagged union has a vpiVoidTypespec.
// - vpiPacked of a packed tagged union is TRUE.
// - A virtual interface property has no typespec: vpiTypespec gives NULL
//   with an error.
//
// IEEE 1800-2009 37.23 detail 1 (SystemVerilog-1800-2009.txt
// L59170-59172): "If a typespec denotes a type that has a user-defined
// typedef, the vpiName property shall return the name of that type;
// otherwise, except in the case of a class typespec (see 37.28), the
// vpiName property shall return NULL. Consequently the vpiName property
// returns NULL for any SystemVerilog built-in type." Detail 6
// (L59200-59206): a typedef of a built-in type has it as vpiTypedefAlias.
// The 37.23 diagram (L59102-59152) lists chandle, void, struct and union
// typespecs with vpiPacked and vpiTagged, and no interface typespec; 6.13
// (L5543-5544) allows void for members of tagged unions; 7.3.2
// (L7943-7944) makes packed a qualifier of a tagged union. Industry-
// standard behaviour names a chandle typespec after the object it was
// reached from and gives it no alias, names an enum's base typespec after
// the enum (an int base too), lists no typespec members, and serves a
// typespec of a vendor-specific type for a virtual interface; llg follows
// the text for all of them.
//
// Build A19-D90_typespec_kinds.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d90(input int)` declared as a system task).
`timescale 1ns/1ns
interface bus_if;
  logic v;
endinterface

module tb;
  typedef chandle ch_t;
  typedef bit [3:0] nib_t;
  typedef enum nib_t {A, B} en_t;
  typedef enum int {I0, I1} ei_t;
  typedef union tagged packed { void Invalid; bit [7:0] Valid; } vp_t;
  class C;
    virtual bus_if vif;
  endclass

  bus_if bi();
  chandle h;
  ch_t hn;
  en_t en;
  ei_t ei;
  vp_t vp;
  C c;

  initial begin
    c = new;
    c.vif = bi;
    vp = tagged Valid 8'h5;
    $d90(0);
    $display("done %0d", vp.Valid);
    $finish;
  end
endmodule
