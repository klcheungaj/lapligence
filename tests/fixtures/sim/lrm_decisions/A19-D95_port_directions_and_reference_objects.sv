// Decision A19-D95: port directions, the reference objects of `ref` ports and
// the type of a logic variable with several packed dimensions.
// IEEE 1800-2009 37.13 detail 1 (SystemVerilog-1800-2009.txt L58128):
// "vpiDirection returns vpiRef for pass by ref ports or arguments." 37.15
// detail 1 (L58245-58248): "A ref obj represents a declared object or
// sub-element of that object that is a reference to an actual instantiated
// object. A ref obj exists for ports with ref direction, for an interface
// port, a modport port, or for formal task function ref arguments. The
// specific cases for a ref obj are: - A variable, named event, named event
// array that is the lowconn of a ref port"; detail 3 (L58257-58258): "The
// vpiActual relationship always returns the actual instantiated object if the
// ref obj is bound to an actual object at the time of the query"; details 5
// and 6 (L58283-58287): vpiVirtual and vpiGeneric "shall return vpiUndefined
// for all other kinds of ref obj". 37.18 detail 1 (L58922): "vpiPackedArrayVar
// objects shall represent packed arrays of packed struct var, union var, or
// enum var objects." The text gives no direction to an interface, modport or
// null port. Decision: an interface or modport port is vpiInout, a port with
// no internal connection (`.e()`) has no direction (industry-standard
// behaviour), and a `ref` port, its bits and its io declaration are vpiRef
// (the text; the industry-standard simulator reports vpiInout). The low
// connection of a `ref` port and the vpiExpr of its io declaration are a
// vpiRefObj named after the port, whose vpiActual is the high connection and
// whose size, typespec and value are the formal's (it reads and takes
// vpi_put_value); it has no direction or parent, and vpiVirtual and vpiGeneric
// are vpiUndefined. The reference objects of an interface or modport port
// have size 0. An io declaration has the size of its port (A19-D94). A logic
// or bit variable with several packed dimensions is a vpiReg or vpiBitVar
// (an element of an array of them too), still packed; a packed array of
// structures stays a vpiPackedArrayVar (the text; the industry-standard
// simulator reports the element type).
//
// Build A19-D95_port_directions_and_reference_objects.c as a VPI application
// (llg: static archive given to --dpi-lib; `task $d95()` declared as a system
// task).
interface ifc;
  logic s;
  modport mp (input s);
endinterface
typedef struct packed { logic [1:0] a; logic b; } sp_t;
module dut (input logic [1:0][2:0] pk, output bit [1:0][3:0] pb, ifc.mp m, ifc f,
            ref int rf, ref logic [7:0] rl, ref real rr, ref string rs, inout wire w);
  initial pb = 8'h3c;
endmodule
module nul (.e(), a);
  input a;
endmodule
module tb;
  logic [1:0][2:0] pk;
  bit [1:0][3:0] pb;
  int rf;
  logic [7:0] rl;
  real rr;
  string rs;
  wire w;
  logic a;
  sp_t [1:0] sa;
  logic [1:0][3:0] ua [2];
  ifc i0 ();
  ifc i1 ();
  dut u (.pk, .pb, .m(i0.mp), .f(i1), .rf, .rl, .rr, .rs, .w);
  nul n (.a);
  initial begin
    pk = 6'h2a;
    rf = 7;
    rl = 8'h21;
    rr = 2.5;
    rs = "ab";
    a = 1'b1;
    #1 $d95;
    #1 $display("rf=%0d rl=%h rr=%0.2f rs=%s", rf, rl, rr, rs);
    $finish;
  end
endmodule
