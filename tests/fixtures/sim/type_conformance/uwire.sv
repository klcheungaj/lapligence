module tb;
    logic [128:0] source;
    uwire scalar;
    uwire signed [128:0] empty;
    uwire [128:0] whole;
    uwire [128:0] selected;
    uwire [128:0] declared = source;
    assign whole=source;
    assign selected[128:65]=source[128:65];
    assign selected[63:0]=source[63:0];
    if (1) begin : g
        uwire [128:0] empty;
        uwire [128:0] declared=source;
    end
    initial begin
        if (scalar !== 1'bz || empty !== 'z || g.empty !== 'z) $display("FAIL defaults");
        source='1; #1;
        if (whole !== '1 || declared !== '1 || g.declared !== '1 ||
            selected !== {64'hffffffffffffffff, 1'bz, 64'hffffffffffffffff}) $display("FAIL driven");
        source='x; #1;
        if (whole !== 'x || declared !== 'x || selected[64] !== 1'bz) $display("FAIL unknown");
        source='z; #1;
        if (whole !== 'z || selected !== 'z || declared !== 'z || g.declared !== 'z) $display("FAIL released");
        $display("PASS uwire"); $finish;
    end
endmodule
