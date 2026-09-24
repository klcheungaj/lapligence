// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_nba_ref_inout_actuals.sv
// A committed prior-edge NBA value is read and modified through ref and inout formals.
module tb;
    typedef logic [7:0] byte_t;

    logic clk = 1'b0;
    logic started = 1'b0;
    byte_t ref_source = 8'h00;
    byte_t inout_source = 8'h00;
    byte_t seen_ref;
    byte_t changed_ref;
    byte_t seen_inout;
    byte_t changed_inout;

    task automatic capture_ref(ref byte_t value);
        seen_ref = value;
        value = value ^ 8'hff;
        changed_ref = value;
    endtask

    task automatic capture_inout(inout byte_t value);
        seen_inout = value;
        value = value ^ 8'hff;
        changed_inout = value;
    endtask

    always_ff @(posedge clk) begin
        if (!started) begin
            started <= 1'b1;
            ref_source <= 8'h5a;
            inout_source <= 8'h5a;
        end else begin
            capture_ref(ref_source);
            ref_source <= 8'ha5;
            capture_inout(inout_source);
            inout_source <= 8'ha5;
        end
    end

    initial begin
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 clk = 1'b1;
        #1;

        if (seen_ref !== 8'h5a || changed_ref !== 8'ha5 || ref_source !== 8'ha5)
            $fatal(1, "ref call did not observe prior committed NBA value");
        if (seen_inout !== 8'h5a || changed_inout !== 8'ha5 || inout_source !== 8'ha5)
            $fatal(1, "inout call did not observe prior committed NBA value");

        $display("nba-ref-inout=5a>a5,5a>a5 final=a5/a5");
        $finish(0);
    end
endmodule
