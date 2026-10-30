// Decision A19-D33: drivers and loads of nets and variables (IEEE 1800-2009
// 37.16 details 11-16 and 19, 37.21, 37.42 and its detail 1).
// vpiDriver and vpiLoad of a net cover its simulated net: the nets that
// ports join to it bit for bit (23.3.3.7), whose ports are neither drivers
// nor loads; a port with a variable or an expression on the other side is
// one ("vpiDriver shall also return ports that are driven by objects other
// than nets and net bits"; "Complex expressions on input ports which are
// not concatenations shall be considered a load"). vpiLocalDriver and
// vpiLocalLoad keep the drivers and loads in the net's instance and the
// ports connected to it ("output and inout ports are loads, input and inout
// ports are drivers" seen from inside). Continuous assignments, primitive
// terminals and active forces count; a timing check does not (37.42 lists
// none). Each driver or load is returned once (detail 15); a bit query
// returns port bits and continuous assignment bits. A variable's drivers
// are ports, forces and continuous assignments and its loads have no ports
// (37.21); vpiLocalDriver and vpiLocalLoad of a variable are wrong-object
// queries (vpiError, A19-D26). vpiPortInst and vpiPort give the ports a
// net or variable connects as high and low connection.
module child (input wire ci, output wire co, input wire [1:0] cv, output logic cr);
  assign co = ci;
  assign cr = cv[0];
  specify
    $setup(ci, posedge co, 1);
  endspecify
endmodule
module tb;
  wire a, b, c;
  wire [3:0] v;
  logic r;
  logic [1:0] lv = 2'b01;
  assign a = 1'b1;
  assign v[1:0] = {a, a};
  and g (c, a, b);
  child u1 (.ci(a), .co(b), .cv(v[3:2]), .cr(r));
  child u2 (.ci(!a), .co(), .cv(lv), .cr());
  initial begin
    #1 $d33(0);
    force v = 4'h0;
    force r = b;
    #1 $d33(1);
    release v;
    release r;
    #1 $d33(2);
    $display("c=%b r=%b", c, r);
    $finish;
  end
endmodule
