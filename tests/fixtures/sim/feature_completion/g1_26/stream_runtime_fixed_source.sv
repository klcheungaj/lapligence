// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/stream_runtime_fixed_source.sv
// IEEE 1800-2009 11.4.14: a runtime `with` selector on a fixed unpacked array
// used as the stream source packs exactly the selected elements, in declared
// order, for the indexed, indexed-descending, single-index and range forms and
// for both stream directions.
module tb;
    logic [7:0] lanes [0:2];
    logic [7:0] desc [2:0];
    logic [15:0] result;
    integer base;
    integer lo;
    integer hi;
    integer i;

    initial begin
        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        lanes[2] = 8'h33;

        // [base +: 2] selects logical lanes 1 then 2.
        base = 1;
        result = {>>8{lanes with [base +: 2]}};
        $display("plus %h", result);

        // [base -: 2] selects logical lanes 2 then 1.
        base = 2;
        result = {>>8{lanes with [base -: 2]}};
        $display("minus %h", result);

        // A single runtime index selects one element.
        i = 0;
        result = {>>8{lanes with [i]}};
        $display("index %h", result);

        // A runtime range selects logical lanes 1 then 2.
        lo = 1;
        hi = 2;
        result = {>>8{lanes with [lo:hi]}};
        $display("range %h", result);

        // A descending declared array keeps its own element order.
        desc[0] = 8'haa;
        desc[1] = 8'hbb;
        desc[2] = 8'hcc;
        base = 0;
        result = {>>8{desc with [base +: 2]}};
        $display("desc %h", result);

        // Right-to-left streaming reverses the whole source block order.
        base = 0;
        result = {<<8{lanes with [base +: 2]}};
        $display("rev %h", result);

        // A slice size other than the element width reverses whole blocks.
        base = 0;
        result = {<<4{lanes with [base +: 2]}};
        $display("rev4 %h", result);
        $finish(0);
    end
endmodule
