// llg-test-fixture: tests/fixtures/sim/sv4_gmp_integration/sampled_owners.sv
// Wide packed values captured by clocking outputs, inertial continuous
// drivers, sampled-value functions and sequence locals (IEEE 1800-2009
// §14.16, §10.3.3 and §6.10, §16.10).
`timescale 1ns/1ns
module tb;
    logic clk = 0;
    logic [129:0] drive_src = '0;
    logic [129:0] drive_dst = '0;
    logic [129:0] inertial_src = '0;
    wire [129:0] inertial_net;
    logic [129:0] seq_src = '0;
    logic [129:0] expected = '0;
    logic start = 0;
    int failures = 0;
    int matched = 0;

    assign #3 inertial_net = inertial_src;

    default clocking cb @(posedge clk);
        output #1 drive_dst;
    endclocking

    sequence carry(local input logic [129:0] source);
        logic [129:0] saved;
        (start, saved = source + 130'd1) ##1 (saved === expected);
    endsequence

    seen: cover property (@(posedge clk) carry(seq_src)) matched++;

    always #5 clk = ~clk;

    initial begin
        // Clocking output: the value is captured at the drive statement and
        // published at the clocking event plus skew; the source then changes.
        drive_src = {2'b01, {64{1'b1}}, 64'h0};
        ##1 cb.drive_dst <= drive_src;
        drive_src = 130'bx;
        @(posedge clk);
        #2;
        if (drive_dst !== {2'b01, {64{1'b1}}, 64'h0}) begin
            failures++;
            $display("FAIL clocking %h", drive_dst);
        end

        // Inertial delay: a pulse narrower than the delay is rejected; the
        // surviving value is the last scheduled one.
        inertial_src = {130{1'b1}};
        #1 inertial_src = {2'b10, 128'bz};
        #1 inertial_src = {66'h3_0000_0000_0000_0000, 64'h1234};
        #4;
        if (inertial_net !== {66'h3_0000_0000_0000_0000, 64'h1234}) begin
            failures++;
            $display("FAIL inertial %h", inertial_net);
        end

        // A sequence local keeps its wide value across later X writes.
        @(negedge clk);
        seq_src = {2'b11, 128'h1};
        start = 1;
        @(posedge clk);
        #1;
        seq_src = 130'bx;
        start = 0;
        expected = {2'b11, 128'h2};
        @(posedge clk);
        #1;
        if (matched != 1) begin
            failures++;
            $display("FAIL sequence local matched=%0d", matched);
        end

        if (failures == 0) $display("PASS sampled_owners");
        $finish(0);
    end
endmodule
