// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/region_pingpong.sv
// IEEE 1800-2009 §§4.5 and 24.3.1: three handshakes complete inside one time
// slot. Each round goes Reactive -> Re-NBA -> Active -> NBA -> Reactive, so
// the scheduler re-enters both region sets repeatedly before Postponed. Only
// one process is runnable at any point, so the output is fully determined.
program p;
    initial begin
        #1;
        repeat (3) begin
            tb.req <= tb.req + 1;
            @(tb.ack);
            $display("P t=%0d req=%0d ack=%0d", $time, tb.req, tb.ack);
        end
        $strobe("S t=%0d req=%0d ack=%0d", $time, tb.req, tb.ack);
        #1;
    end
endprogram

module tb;
    int req = 0;
    int ack = 0;
    p p0();

    always @(req) ack <= req * 10;
endmodule
