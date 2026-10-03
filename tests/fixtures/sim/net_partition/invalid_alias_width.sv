// llg-test-fixture: electrical partitioning must retain alias-width legality.
module tb;
    wire [128:0] array_net[1];
    wire [127:0] peer;
    alias peer = array_net[0];
endmodule
