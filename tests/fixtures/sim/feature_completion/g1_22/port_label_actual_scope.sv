// llg-test-fixture: IEEE 1800-2009 23.2.2. A named port connection's label
// names the child formal while the actual expression is resolved in the
// parent scope, even when both declarations share a spelling. Reading the
// child declaration and the parent declaration must stay distinct.
module child(input logic value, output logic result);
    assign result = value;
endmodule

module mid(input logic value, output logic result);
    child u_child(.value(value), .result(result));
endmodule

module tb;
    logic value;   // parent declaration spelled like the child port label
    logic result;
    mid u_mid(.value(value), .result(result));

    initial begin
        value = 1'b1;
        #1 $display("p=%b c=%b r=%b", value, u_mid.u_child.value, result);
        value = 1'b0;
        #1 $display("p=%b c=%b r=%b", value, u_mid.u_child.value, result);
        $finish(0);
    end
endmodule
