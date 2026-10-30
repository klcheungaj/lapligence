// Decision A19-D28: index, range and select relations of nets and
// variables. vpi_iterate(vpiIndex) of an array element returns its indices
// innermost first, and of a bit of an array element the bit's index
// followed by the indices of the element that holds it. Outside an array,
// vpi_iterate(vpiIndex) of a net or named event returns NULL without an
// error, as does that of a whole variable array or packed array; of another
// variable, or of a bit of a vector, it returns NULL with a vpiError, and
// vpi_handle(vpiIndex) of a whole net, variable or named event returns NULL
// with a vpiError. Each variable type answers through the elements of an
// unpacked array of it. vpi_iterate(vpiRange) of a logic or
// bit vector, a net or a packed array returns its packed ranges leftmost
// first (NULL for a scalar). vpiConstantSelect is TRUE for every net, net
// bit, var bit, packed array, structure and enum variable the design
// declares (static, selected by constant indices). vpiPackedArrayMember is
// FALSE for a declared variable; the sub-elements of a packed array
// (vpiElement) are not objects. vpiVarSelect iterates the elements of a
// variable array as vpiReg does. vpiScalar and vpiVector are FALSE for real
// and string variables. An element of an instance array has the array as
// vpiInstanceArray and vpiModuleArray (vpiPrimitiveArray for a gate, a UDP
// or a switch); outside an array vpiInstanceArray returns NULL with a
// vpiError and vpiPrimitiveArray NULL without one. A net bit has its net's vpiNetType and
// a port bit its port's vpiPortType.
//
// IEEE 1800-2009 37.16 detail 22 (SystemVerilog-1800-2009.txt L58563-
// L58565): "vpi_iterate(vpiIndex, net_handle) shall return the set of
// indices for a net within an array net, starting with the index for the
// net and working outward. If the net is not part of an array (the
// vpiArrayMember property is FALSE), a NULL shall be returned."; 37.25
// detail 1 (L59330-L59332) says the same of a named event; 37.16 detail 10
// (L58515-L58517) and 37.17 detail 13 (L58769-L58771): "vpi_iterate
// (vpiIndex, var_bit_handle) shall return the set of indices for a
// multidimensional variable bit select, starting with the index for the
// bit and working outwards."; 37.17 detail 5 (L58753): a var select's
// indices start "with the index for the var select and working outward".
// 37.5 (diagram, L57769-L57770) and 37.31 relate a module and a primitive
// to their instance array; 37.13 detail 7 (L58190): "Properties vpiIndex
// and vpiName shall not apply for port bits", so vpiPortType does. The
// text defines no index for a whole variable, for a bit of a vector
// outside an array, or for vpi_handle(vpiIndex) of a whole object, and no
// instance array for an instance outside an array; there llg follows
// common simulator practice. 37.17 detail 4 (L58747-L58748): "For a packed
// array, the iteration shall return the set of ranges beginning with the
// leftmost packed range"; 37.16 detail 20 (L58549) and 37.17 detail 27
// (L58887): vpiConstantSelect "shall return TRUE for a var bit or other
// variable if it has a static lifetime and has no parent"; 37.18 detail 4
// (L58957): "The vpiPackedArrayMember property shall be TRUE for any struct
// var, union var, enum var, or packed array var whose vpiParent is a packed
// array var"; 37.17 detail 7 (L58757): "A var select is an element selected
// from an array var."; detail 20 (L58817-L58818): "The vpiScalar and
// vpiVector properties shall return FALSE for all other var objects." Some
// simulators return the indices outermost first and only the bit index for
// a bit of an array element, report an error for the iteration of a whole
// net or named event, and refuse vpiConstantSelect and
// vpiPackedArrayMember on whole variables, the instance array of an element
// and vpiPortType of a port bit; llg follows the clause text.
//
// Build A19-D28_index_and_packed_relations.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d28()` declared as a system
// task).
module leaf (input logic [1:0] a);
endmodule
primitive udp_and (out, a, b);
  output out;
  input a, b;
  table
    0 ? : 0;
    ? 0 : 0;
    1 1 : 1;
  endtable
endprimitive
module tb;
  typedef struct packed { logic [3:0] hi; bit [3:0] lo; } pk_t;
  typedef enum bit [1:0] {A, B, C} e_t;
  typedef union packed { logic [7:0] w; pk_t p; } pu_t;
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
  event ev;
  // An unpacked array of each variable type: its elements have an index.
  bit [3:0] bva [0:1];
  byte bya [0:1];
  shortint sia [0:1];
  int iva [0:1];
  longint lia [0:1];
  integer iga [0:1];
  time tva [0:1];
  real rva [0:1];
  shortreal sra [0:1];
  e_t eva [0:1];
  pk_t psa [0:1];
  pu_t pua [0:1];
  logic [1:0][3:0] paa [0:1];
  wire [1:0] uy, ta, tb2;
  wire uo;
  leaf u [1:0] (.a(v[1:0]));
  leaf single (.a(v[3:2]));
  and g [1:0] (y, v[1:0], v[3:2]);
  udp_and ug [1:0] (uy, v[1:0], v[3:2]);
  udp_and us (uo, v[4], v[5]);
  tran tg [1:0] (ta, tb2);
  assign w = v[3:0];
  initial begin
    #1 $d28;
    $finish;
  end
endmodule
