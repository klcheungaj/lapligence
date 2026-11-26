// Decision A19-D94: the size of a port whose value has no packed width of
// its own. IEEE 1800-2009 37.14 draws "int: vpiSize", "bool: vpiScalar" and
// "bool: vpiVector" on ports, and detail 6 (SystemVerilog-1800-2009.txt
// L58188-58189): "Properties vpiScalar and vpiVector shall indicate if the
// port is 1 bit or more than 1 bit. They shall not indicate anything about
// what is connected to the port." The text gives no size for an interface,
// unpacked or string port; for variables, 37.17 detail 9 (L58764) ends "For
// all other variables, the behavior of the vpiSize property is not
// defined." Decision, as industry-standard behaviour: an
// interface or modport port and an unpacked structure port are 1 bit
// (scalar); an unpacked array port has the size of one element (a real
// element 64 bits, an unpacked structure element 1, a string element 64); a
// string port's size is vpiUndefined. vpiScalar is TRUE for a size of 1 and
// vpiVector for a larger one; a string port and a port of an array of
// strings are vectors. A packed port keeps its width.
//
// Build A19-D94_port_sizes.c as a VPI application (llg: static archive given
// to --dpi-lib; `task $d94()` declared as a system task).
interface ifc;
  logic s;
  logic [3:0] w;
  modport mp (input s, input w);
endinterface
typedef struct packed { logic [2:0] a; logic b; } sp_t;
typedef struct { logic [2:0] a; int b; logic c; } su_t;
module dut (input logic s, input logic [3:0] v, input logic [1:0][2:0] pk,
            input logic ua [2], input logic [3:0] ub [3][2], input int ui [0:3],
            input real ra [2], input sp_t spa [3], input logic [1:0][2:0] pa [2],
            input su_t su, input su_t sua [2], output logic [2:0] o [2],
            ifc.mp m, ifc f, input string str, input string sa [2]);
  initial o = '{3'd1, 3'd2};
endmodule
module tb;
  logic s; logic [3:0] v; logic [1:0][2:0] pk; logic ua [2]; logic [3:0] ub [3][2];
  int ui [0:3]; real ra [2]; sp_t spa [3]; logic [1:0][2:0] pa [2]; su_t su; su_t sua [2];
  logic [2:0] o [2]; string str; string sa [2];
  ifc i0 ();
  ifc i1 ();
  dut u (.s, .v, .pk, .ua, .ub, .ui, .ra, .spa, .pa, .su, .sua, .o, .m(i0.mp), .f(i1), .str, .sa);
  initial begin
    #1;
    $d94;
    $finish;
  end
endmodule
