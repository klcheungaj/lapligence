// llg-test-fixture: ADV-017 A01/A02. lifecycle.c ($attach, $ctl, $report)
// places assertion callbacks (IEEE 1800-2009 39.4.2) on every assertion of
// a design with a multiclock, a global clocking future, a checker-owned and
// a cover assertion, controls one of them through vpi_control (39.5.2), and
// correlates the callbacks with the HDL action counters and the 40.5.3
// coverage counters. Expected output: lifecycle.out (derivation in
// readme.md).
`timescale 1ns/1ns
checker handshake(logic req, logic ack, event ck);
    c_ack: assert property (@ck req |=> ack);
    c_req: cover property (@ck req ##1 ack);
endchecker

module tb;
    logic clk = 0, clk2 = 0, gclk = 0;
    logic req = 0, ack = 0, rst = 0;
    int a_pass = 0, a_fail = 0, m_pass = 0, m_fail = 0, f_pass = 0, f_fail = 0;
    int cov_hits = 0;

    always #5 clk = ~clk;
    always #3 clk2 = ~clk2;
    always #1 gclk = ~gclk;
    global clocking @(posedge gclk); endclocking

    a_req: assert property (@(posedge clk) disable iff (rst) req |=> ack) a_pass++;
        else a_fail++;
    m_two: assert property (@(posedge clk) req |=> @(posedge clk2) ack) m_pass++;
        else m_fail++;
    f_fut: assert property (@(posedge clk) $rose(req) |-> $future_gclk(req)) f_pass++;
        else f_fail++;
    k_cov: cover property (@(posedge clk) req ##1 ack) cov_hits++;
    handshake u_hs(req, ack, posedge clk);

    initial begin
        $attach;
        #2 req = 1'b1;
        #10 ack = 1'b1;
        #10 ack = 1'b0;
        #6 rst = 1'b1;
        #4 rst = 1'b0;
        #6 $ctl(1);
        #4 req = 1'b0;
        #2 $ctl(2);
        #8 req = 1'b1;
        ack = 1'b1;
        #18 $report;
        $finish;
    end
endmodule
