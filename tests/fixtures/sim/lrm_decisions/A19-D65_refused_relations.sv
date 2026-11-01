// Decision A19-D65: relations of the 1800-2009 object diagrams that are
// refused with an error. IEEE 1800-2009 37.77 (L61505-61515) draws
// vpiActiveTimeFormat, 37.71 (L61372) alias statements, 37.53 (L60745)
// let declarations, 37.23 typespecs with vpiInstance, vpiModule and
// vpiScope and enum constants with vpiEnumTypespec, 37.12 (L58028-58060)
// vpiMemory of every scope, 37.27-37.29 vpiTypedef, vpiTypedefAlias and
// vpiParamAssign of classes and vpiArgument of an extends clause; 38.19
// gives vpi_handle_by_index to objects with an index. The text names no
// value for arrays, function calls or class variables and no integer
// vpiName.
// Decision: industry-standard behaviour refuses each query below with an
// error, and so does llg: an alias statement, a let declaration and a type
// parameter are not objects; a typespec has no instance, module or scope;
// an enum constant does not lead back to its typespec; a class lists its
// properties through vpiVariables, not vpiMemory, and no typedefs, typedef
// alias, specialization parameter assignments or base-class arguments; a
// port is not indexed; an array, a function call and a class variable have
// no value; vpi_get(vpiName) of a string property is an error.
`timescale 1ns/1ns
interface ifc;
  logic s;
  modport mp (input s);
endinterface
module user (ifc.mp p);
  wire t = p.s;
endmodule
class Base;
  int b;
  function new(int v); b = v; endfunction
endclass
class Derived #(parameter int P = 4) extends Base;
  typedef logic [P-1:0] word_t;
  word_t w;
  function new(); super.new(P); endfunction
endclass
module tb;
  typedef enum logic [1:0] { RED, GREEN } color_t;
  typedef struct packed { logic [3:0] f; } s_t;
  wire [3:0] a1, a2;
  alias a1 = a2;
  let inc(x) = x + 1;
  color_t c;
  s_t sv;
  logic [3:0] mem [0:1];
  wire [3:0] nets [0:1];
  ifc i0 ();
  user u (.p(i0.mp));
  Derived #(8) d;
  function int twice(input int v); return 2 * v; endfunction
  int r;
  initial begin
    d = new;
    r = twice(inc(1));
    $timeformat(-9, 2, " ns", 10);
    #1 $d65;
  end
endmodule
