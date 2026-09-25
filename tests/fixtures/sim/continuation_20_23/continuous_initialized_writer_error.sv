// llg-test-fixture: SYN-006 a declaration initializer is a procedural assignment.
module tb;
    logic [64:0] source[2] = '{65'd1, 65'd2};
    logic [64:0] destination[2] = '{65'd0, 65'd0};
    assign destination = source;
endmodule
