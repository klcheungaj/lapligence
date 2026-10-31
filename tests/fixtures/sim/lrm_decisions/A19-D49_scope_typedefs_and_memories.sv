// Decision A19-D49: typedefs and memories of a scope. vpi_iterate(
// vpiTypedef, scope) returns the typedefs the module, interface, program,
// package, generate scope, task, function or named block itself declares,
// in declaration order, as the typespecs that name them (vpiName, the
// typedef's line as vpiLineNo, vpiTypedefAlias for a typedef of another
// typedef or of a built-in type); NULL when it declares none. vpi_iterate(
// vpiMemory, scope) returns the scope's memories: its one-dimensional
// arrays of reg (logic) variables, whose vpiIsMemory is TRUE, as
// vpiRegArray objects.
//
// IEEE 1800-2009 37.10 detail 1 (SystemVerilog-1800-2009.txt L57932-57933):
// "The vpiTypedef iteration shall return the user-defined typespecs that
// have typedefs explicitly declared in the instance." Detail 4 (L57937):
// "vpiMemory shall return array variable objects rather than vpiMemory
// objects." 37.20 detail 1 (L59036-59038): vpiMemory and vpiMemoryWord
// "have been converted into methods which will return objects of type
// vpiRegArray and vpiReg respectively." 6.11.2: logic and reg name the
// same type.
//
// Build A19-D49_scope_typedefs_and_memories.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d49()` declared as a system
// task).
`timescale 1ns/1ns
package pk;
  typedef logic [7:0] byte_t;
endpackage
module child;
  typedef enum logic [1:0] {X0, X1} ce_t;
  ce_t c;
endmodule
module tb;
  typedef logic [3:0] used_t;
  typedef int unused_t;
  typedef struct packed { logic a; used_t b; } st_t;
  typedef used_t alias_t;
  typedef pk::byte_t imp_t;
  used_t v;
  st_t s;
  reg [3:0] mem [0:3];
  reg [3:0] mem2 [0:1][0:1];
  integer imem [0:1];
  logic [3:0] lmem [0:1];
  child u ();
  initial begin : blk
    typedef bit inner_t;
    inner_t q;
    q = 1;
  end
  initial #1 $d49;
endmodule
