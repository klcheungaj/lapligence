// llg-test-fixture: tests/fixtures/sim/process_semantics/at_star_distinct.sv
// IEEE 1364-2001 §9.7.5 and IEEE 1800-2009 §9.2.2.2: @* uses call-site
// sensitivity, while always_comb includes reads made by called functions.
module tb;
    logic a;
    logic y_at;
    logic y_comb;

    function automatic logic hidden();
        hidden = a;
    endfunction

    always @* y_at = hidden();
    always_comb y_comb = hidden();

    initial begin
        a = 1'b0;
        #1 $display("t=%0t at=%b comb=%b", $time, y_at, y_comb);
        a = 1'b1;
        #1 $display("t=%0t at=%b comb=%b", $time, y_at, y_comb);
        $finish;
    end
endmodule
