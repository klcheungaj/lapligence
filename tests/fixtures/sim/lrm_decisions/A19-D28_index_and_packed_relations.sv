// Decision A19-D28: index, range and select relations of nets and
// variables. vpi_iterate(vpiIndex) of an array element returns its indices
// innermost first, and of a bit the bit's index followed by the indices of
// the element that holds it; a whole net or variable outside an array has
// no index: vpi_handle(vpiIndex) and vpi_iterate(vpiIndex) return NULL
// without an error. vpi_iterate(vpiRange) of a logic or bit vector, a net
// or a packed array returns its packed ranges leftmost first (NULL for a
// scalar). vpiConstantSelect is TRUE for every net, net bit, var bit, packed
// array, structure and enum variable the design declares (static, selected
// by constant indices). vpiPackedArrayMember is FALSE for a declared
// variable; the sub-elements of a packed array (vpiElement) are not objects.
// vpiVarSelect iterates the elements of a variable array as vpiReg does.
// vpiScalar and vpiVector are FALSE for real and string variables. An
// element of an instance array has the array as vpiInstanceArray and
// vpiModuleArray (vpiPrimitiveArray for a gate), and an instance outside an
// array NULL. A net bit has its net's vpiNetType and a port bit its port's
// vpiPortType.
//
// IEEE 1800-2009 37.16 detail 22 (SystemVerilog-1800-2009.txt L58563-
// L58565): "vpi_iterate(vpiIndex, net_handle) shall return the set of
// indices for a net within an array net, starting with the index for the
// net and working outward. If the net is not part of an array (the
// vpiArrayMember property is FALSE), a NULL shall be returned."; 37.17
// detail 13 (L58769-L58771): "vpi_iterate(vpiIndex, var_bit_handle) shall
// return the set of indices for a multidimensional variable bit select,
// starting with the index for the bit and working outwards."; detail 4
// (L58747-L58748): "For a packed array, the iteration shall return the set
// of ranges beginning with the leftmost packed range"; 37.16 detail 20
// (L58549) and 37.17 detail 27 (L58887): vpiConstantSelect "shall return
// TRUE for a var bit or other variable if it has a static lifetime and has
// no parent"; 37.18 detail 4 (L58957): "The vpiPackedArrayMember property
// shall be TRUE for any struct var, union var, enum var, or packed array var
// whose vpiParent is a packed array var"; 37.17 detail 7 (L58757): "A var
// select is an element selected from an array var."; detail 20 (L58817-
// L58818): "The vpiScalar and vpiVector properties shall return FALSE for
// all other var objects." The text is silent on vpi_handle(vpiIndex) of an
// object outside an array; llg returns NULL as for the iteration. Some
// simulators return the indices outermost first and only the bit index for
// a bit, refuse vpiIndex, vpiConstantSelect and vpiPackedArrayMember on
// whole variables, the instance array of an element and vpiPortType of a
// port bit; llg follows the clause text.
//
// Build A19-D28_index_and_packed_relations.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d28()` declared as a system
// task).
module leaf (input logic [1:0] a);
endmodule
module tb;
  typedef struct packed { logic [3:0] hi; bit [3:0] lo; } pk_t;
  typedef enum bit [1:0] {A, B, C} e_t;
  logic [7:0] v = 8'h5a;
  logic sc = 1'b1;
  logic [1:0][3:0] pa = 8'h12;
  pk_t ps = 8'h56;
  e_t en = B;
  real rv = 1.5;
  string s = "x";
  logic [3:0] mem [0:3];
  logic [3:0] mem2 [0:1][2:3];
  wire [3:0] w;
  wire [1:0] y;
  leaf u [1:0] (.a(v[1:0]));
  leaf single (.a(v[3:2]));
  and g [1:0] (y, v[1:0], v[3:2]);
  assign w = v[3:0];
  initial begin
    #1 $d28;
    $finish;
  end
endmodule
