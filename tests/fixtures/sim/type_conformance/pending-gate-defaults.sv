module leaf(inout wire [64:0] bus, input logic [64:0] a);
    not #2 g(bus,a);
endmodule
module tb;
    logic [64:0] source;
    wire [64:0] direct_value, linked_value, empty;
    not #2 g(direct_value,source);
    leaf child(linked_value,source);
    initial begin
        source='0; #1;
        if (direct_value !== 'x || linked_value !== 'x || empty !== 'z) $display("FAIL pending gate");
        #2;
        if (direct_value !== '1 || linked_value !== '1 || empty !== 'z) $display("FAIL first update");
        source='1; #1;
        if (direct_value !== '1 || linked_value !== '1) $display("FAIL pending transition");
        #2;
        if (direct_value !== '0 || linked_value !== '0) $display("FAIL second update");
        $display("PASS pending gates"); $finish(0);
    end
endmodule
