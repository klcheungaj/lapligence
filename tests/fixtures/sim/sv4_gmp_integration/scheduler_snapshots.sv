// llg-test-fixture: tests/fixtures/sim/sv4_gmp_integration/scheduler_snapshots.sv
// Packed owners retained by the scheduler keep independent snapshots when the
// source changes after capture (IEEE 1800-2009 §10.4.2, §9.4.5, §10.6, §16.9.3).
// Values cross the inline/wide boundary and the known/X-Z boundary so a
// backend that relocates or reshapes payloads must still publish the capture.
`timescale 1ns/1ns
module tb;
    logic [129:0] wide_src, wide_dst, wide_sel, wide_force;
    logic [63:0] narrow_src, narrow_dst;
    logic [129:0] wide_net_src;
    wire [129:0] wide_net = wide_net_src;
    logic clk = 0;
    logic [129:0] hist;
    int failures = 0;

    task automatic check(input string name, input logic [129:0] got,
                         input logic [129:0] want);
        if (got !== want) begin
            failures++;
            $display("FAIL %s got=%h want=%h", name, got, want);
        end
    endtask

    always @(posedge clk) hist <= wide_src;

    initial begin
        // NBA with intra-assignment delay captures the RHS at issue.
        wide_src = {2'b10, 64'h0123_4567_89ab_cdef, 64'hfedc_ba98_7654_3210};
        wide_dst = '0;
        wide_dst <= #2 wide_src;
        wide_src = 130'bx;
        narrow_src = 64'hdead_beef_0000_0001;
        narrow_dst <= #2 narrow_src;
        narrow_src = 64'hz;
        #1;
        check("nba_wide_pending", wide_dst, 130'd0);
        #2;
        check("nba_wide", wide_dst,
              {2'b10, 64'h0123_4567_89ab_cdef, 64'hfedc_ba98_7654_3210});
        check("nba_narrow", {66'd0, narrow_dst}, {66'd0, 64'hdead_beef_0000_0001});

        // Selected NBA into an X/Z-carrying wide target; the source becomes
        // known afterwards and the target keeps the captured unknown slice.
        wide_sel = {130{1'bz}};
        wide_src = {65'h1_ffff_ffff_ffff_ffff, 65'bx};
        wide_sel[100 -: 70] <= #1 wide_src[69:0];
        wide_src = '0;
        #2;
        check("selected_nba", wide_sel,
              {{29{1'bz}}, {5{1'b1}}, {65{1'bx}}, {31{1'bz}}});

        // Force captures a continuous expression; release resumes the source.
        wide_net_src = {130{1'b1}};
        #1;
        force wide_net = {66'h0, 64'h5555_aaaa_5555_aaaa};
        wide_net_src = '0;
        #1;
        check("force_hold", wide_net, {66'h0, 64'h5555_aaaa_5555_aaaa});
        release wide_net;
        #1;
        check("force_release", wide_net, 130'd0);

        // Edge-sampled history: the NBA from posedge sees the old source.
        wide_src = {130{1'b1}};
        #1 clk = 1;
        #0 wide_src = {2'b01, 128'h0};
        #1;
        check("posedge_capture", hist, {130{1'b1}});
        clk = 0;
        #1;

        // Same-variable self-assignments with overlapping slices.
        wide_force = {2'b11, 64'h1111_2222_3333_4444, 64'h5555_6666_7777_8888};
        wide_force[129:10] = wide_force[119:0];
        check("overlap_up", wide_force,
              {2'b00, 64'h4488_88cc_cd11_1155, 64'h5599_99dd_de22_2088});
        wide_force[119:0] = wide_force[129:10];
        check("overlap_down", wide_force,
              {2'b00, 64'h4411_2222_3333_4444, 64'h5555_6666_7777_8888});
        wide_force = {wide_force[64:0], wide_force[129:65]};
        check("rotate", wide_force,
              {2'b00, 64'haaaa_cccc_eeef_1110, 64'h2208_9111_1999_a222});

        if (failures == 0) $display("PASS scheduler_snapshots");
        $finish(0);
    end
endmodule
