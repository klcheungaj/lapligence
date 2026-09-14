// R28: a propagation commit must publish aliases without a polling read.
`timescale 1ns/1ns
module tb;
    logic driver = 0;
    wire #2 a;
    wire b;
    alias a = b;
    assign a = driver;
    int edge_count = 0, level_count = 0;
    time edge_time = 0, level_time = 0;
    initial begin
        #3;
        @(posedge b);
        edge_count++;
        edge_time = $time;
    end
    initial begin
        #3;
        wait (b === 1'b1);
        level_count++;
        level_time = $time;
    end
    initial begin
        #4 driver = 1;
        #3;
        $display("edge=%0d@%0d level=%0d@%0d", edge_count, edge_time,
                 level_count, level_time);
        $finish(0);
    end
endmodule
