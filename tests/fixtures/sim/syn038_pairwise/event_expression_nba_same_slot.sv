// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/event_expression_nba_same_slot.sv
// IEEE 1800-2009 §§4.4, 9.2.2.2 and 9.4.2.1.
module tb;
    logic clk = 1'b0;
    logic [7:0] focal = 8'h11;
    logic [7:0] next_value = 8'h22;
    logic [7:0] pre_nba_sample = 8'h00;
    logic [7:0] event_sample = 8'h00;
    logic event_seen = 1'b0;

    always_ff @(posedge clk) begin
        pre_nba_sample <= focal;
        focal <= next_value;
    end

    initial begin
        fork
            begin
                @(focal);
                event_sample = focal;
                event_seen = 1'b1;
            end
            begin
                #1;
                if (focal !== 8'h11 ||
                    pre_nba_sample !== 8'h00 ||
                    event_sample !== 8'h00 ||
                    event_seen !== 1'b0)
                    $fatal(1, "slot or event waiter changed before the clock edge");
                $display("before=%h old=%h event=%h seen=%0d", focal, pre_nba_sample, event_sample, event_seen);
                clk = 1'b1;
            end
        join

        if (pre_nba_sample !== 8'h11 ||
            focal !== 8'h22 ||
            event_sample !== 8'h22 ||
            event_seen !== 1'b1)
            $fatal(1, "event waiter did not observe the committed NBA value");
        $display("after=%h state=%h event=%h seen=%0d", pre_nba_sample, focal, event_sample, event_seen);
        $finish(0);
    end
endmodule
