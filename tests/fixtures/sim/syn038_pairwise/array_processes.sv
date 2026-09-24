// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/array_processes.sv
// IEEE 1800-2009 §§9.2.2.2, 9.2.2.4, 10.4.2, and 11.4.11: selected fixed-array
// elements are legal procedural targets in combinational and sequential processes.
module tb;
    typedef struct {
        logic [7:0] payload;
        bit valid;
    } record_t;

    logic [7:0] comb_result [0:1];
    logic comb_index;
    logic comb_selector;
    logic [7:0] comb_yes;
    logic [7:0] comb_no;

    logic clk;
    logic nba_index;
    logic nba_selector;
    logic [7:0] nba_yes;
    logic [7:0] nba_no;
    logic [7:0] nba_result [0:1];

    logic record_index;
    logic record_selector;
    record_t record_yes;
    record_t record_no;
    record_t record_result [0:1];

    function automatic logic [7:0] choose_byte(
        input logic selector,
        input logic [7:0] selected,
        input logic [7:0] fallback_value
    );
        return selector ? selected : fallback_value;
    endfunction

    always_comb comb_result[comb_index] = choose_byte(comb_selector, comb_yes, comb_no);

    always_ff @(posedge clk) begin
        nba_result[nba_index] <= nba_selector ? nba_yes : nba_no;
        record_result[record_index] <= record_selector ? record_yes : record_no;
    end

    initial begin
        comb_index = 0;
        comb_selector = 0;
        comb_yes = 8'ha5;
        comb_no = 8'h5a;

        clk = 0;
        nba_index = 0;
        nba_selector = 1;
        nba_yes = 8'h5a;
        nba_no = 8'ha5;
        record_index = 0;
        record_selector = 1;
        record_yes.payload = 8'hd3;
        record_yes.valid = 1;
        record_no.payload = 8'h3c;
        record_no.valid = 0;

        #1;
        if (comb_result[0] !== 8'h5a || comb_result[1] !== 8'hxx)
            $fatal(1, "always_comb selected element, initial selector");

        comb_index = 1;
        comb_selector = 1;
        comb_yes = 8'hc3;
        comb_no = 8'hd4;
        #1;
        if (comb_result[0] !== 8'h5a || comb_result[1] !== 8'hc3)
            $fatal(1, "always_comb selected element, changed index");
        $display("comb=%h,%h", comb_result[0], comb_result[1]);

        clk = 1;
        #0;
        nba_index = 1;
        nba_selector = 0;
        nba_yes = 8'h33;
        nba_no = 8'h44;
        record_index = 1;
        record_selector = 0;
        record_yes.payload = 8'h18;
        record_yes.valid = 0;
        record_no.payload = 8'h19;
        record_no.valid = 1;
        #1;
        if (nba_result[0] !== 8'h5a || nba_result[1] !== 8'hxx)
            $fatal(1, "always_ff first element target and source capture");
        if (record_result[0].payload !== 8'hd3 || record_result[0].valid !== 1'b1 ||
            record_result[1].payload !== 8'hxx)
            $fatal(1, "always_ff first record target and source capture");

        clk = 0;
        nba_index = 1;
        nba_selector = 0;
        nba_yes = 8'h33;
        nba_no = 8'h44;
        record_index = 1;
        record_selector = 0;
        record_yes.payload = 8'h21;
        record_yes.valid = 0;
        record_no.payload = 8'h66;
        record_no.valid = 0;
        #1;

        clk = 1;
        #0;
        nba_index = 0;
        nba_selector = 1;
        nba_yes = 8'h77;
        nba_no = 8'h88;
        record_index = 0;
        record_selector = 1;
        record_yes.payload = 8'h77;
        record_yes.valid = 1;
        record_no.payload = 8'h88;
        record_no.valid = 0;
        #1;

        if (nba_result[0] !== 8'h5a || nba_result[1] !== 8'h44)
            $fatal(1, "always_ff second element target and source capture");
        if (record_result[0].payload !== 8'hd3 || record_result[0].valid !== 1'b1 ||
            record_result[1].payload !== 8'h66 || record_result[1].valid !== 1'b0)
            $fatal(1, "always_ff second record target and source capture");
        $display("ff=%h,%h records=%h/%b,%h/%b", nba_result[0], nba_result[1],
                 record_result[0].payload, record_result[0].valid,
                 record_result[1].payload, record_result[1].valid);
        $finish(0);
    end
endmodule
