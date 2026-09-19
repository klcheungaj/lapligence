// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/stream_dynamic_fixed_selection.sv
// IEEE 1800-2009 11.4.14: a runtime `with` selector on a fixed unpacked array
// destination selects the requested elements exactly once, in the declared
// array order, for the indexed, indexed-descending, single and range forms and
// for both stream directions.
module tb;
    logic [7:0] lanes [0:2];
    logic [7:0] desc [2:0];
    integer base;
    integer lo;
    integer hi;
    integer i;

    initial begin
        // [base +: 2] selects logical lanes 1 and 2.
        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        lanes[2] = 8'h00;
        base = 1;
        {>>8{lanes with [base +: 2]}} = 16'ha4_b5;
        $display("plus %h %h %h", lanes[0], lanes[1], lanes[2]);

        // [base -: 2] selects logical lanes 2 then 1.
        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        lanes[2] = 8'h00;
        base = 2;
        {>>8{lanes with [base -: 2]}} = 16'hc1_d2;
        $display("minus %h %h %h", lanes[0], lanes[1], lanes[2]);

        // A single runtime index selects one element.
        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        lanes[2] = 8'h00;
        i = 0;
        {>>8{lanes with [i]}} = 8'hee;
        $display("index %h %h %h", lanes[0], lanes[1], lanes[2]);

        // A runtime range selects logical lanes 1 and 2.
        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        lanes[2] = 8'h00;
        lo = 1;
        hi = 2;
        {>>8{lanes with [lo:hi]}} = 16'h12_34;
        $display("range %h %h %h", lanes[0], lanes[1], lanes[2]);

        // The declared descending range keeps its own element order.
        desc[0] = 8'h00;
        desc[1] = 8'h00;
        desc[2] = 8'h00;
        base = 0;
        {<<8{desc with [base +: 2]}} = 16'h55_66;
        $display("desc %h %h %h", desc[2], desc[1], desc[0]);

        // Reverse streaming visits the same selected elements in reverse.
        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        lanes[2] = 8'h00;
        base = 1;
        {<<8{lanes with [base +: 2]}} = 16'h77_88;
        $display("reverse %h %h %h", lanes[0], lanes[1], lanes[2]);

        // The whole source must be staged before the overlapping stores.
        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        lanes[2] = 8'h33;
        base = 1;
        {>>8{lanes with [base +: 2]}} = {>>8{lanes with [0 +: 2]}};
        $display("overlap %h %h %h", lanes[0], lanes[1], lanes[2]);
        $finish(0);
    end
endmodule
