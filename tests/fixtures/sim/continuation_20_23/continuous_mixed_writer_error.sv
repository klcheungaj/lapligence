// llg-test-fixture: SYN-006 a procedural write cannot overlap a continuous array writer.
module tb;
    logic [64:0] source[2];
    logic [64:0] destination[2];
    assign destination = source;
    initial destination[1] = 65'd1;
endmodule
