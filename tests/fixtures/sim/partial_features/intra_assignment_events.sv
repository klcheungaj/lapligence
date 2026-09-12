// llg-test-fixture: tests/fixtures/sim/partial_features/intra_assignment_events.sv
`timescale 1ns/1ns
module tb;
    reg clk;
    reg [7:0] blocking, b, repeated;
    reg [7:0] deferred;
    reg [7:0] zero_count, negative_count, unknown_count;
    reg [2:0] index;
    integer negative;

    initial begin
        clk = 1'b0;
        b = 8'h11;
        blocking = 8'h00;
        repeated = 8'h00;
        deferred = 8'h00;
        index = 3'd0;
        zero_count = 8'h00;
        negative_count = 8'h00;
        unknown_count = 8'h00;
        negative = -1;
        unknown_count = 8'hxx;

        blocking[index] = @(posedge clk) b;
        $display("blocking t=%0t value=%h b=%h index=%0d", $time, blocking, b, index);
    end

    initial begin
        deferred[index] <= @(posedge clk) 1'b1;
        $display("nba caller t=%0t index=%0d deferred=%h", $time, index, deferred);
    end

    initial begin
        repeated = repeat (2) @(posedge clk) b;
        $display("repeat t=%0t repeated=%h", $time, repeated);
    end

    initial begin
        zero_count <= repeat (0) @(posedge clk) 8'ha0;
        negative_count <= repeat (negative) @(posedge clk) 8'hb0;
        unknown_count <= repeat (unknown_count) @(posedge clk) 8'hc0;
        #1 $display("zero t=%0t zero=%h negative=%h unknown=%h", $time,
                    zero_count, negative_count, unknown_count);
    end

    initial begin
        #1 b = 8'h22;
        #1 index = 3'd3;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #3 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $display("final t=%0t blocking=%h repeated=%h deferred=%h index=%0d", $time,
                    blocking, repeated, deferred, index);
        #1 $finish;
    end
endmodule
