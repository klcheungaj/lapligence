module tb;
    logic [64:0] source;
    wire [64:0] wires[0:1];
    tri [64:0] tris[0:1];
    uwire [64:0] singles[0:1];
    assign wires[0]=source;
    assign tris[0]=source;
    assign singles[0]=source;
    initial begin
        source={1'b1, 32'hxxzz10ff, 32'hzzxx0180}; #1;
        if (wires[0] !== source || tris[0] !== source || singles[0] !== source ||
            wires[1] !== 'z || tris[1] !== 'z || singles[1] !== 'z) $display("FAIL array drivers");
        source='z; #1;
        if (wires[0] !== 'z || tris[0] !== 'z || singles[0] !== 'z) $display("FAIL array release");
        $display("PASS net arrays"); $finish(0);
    end
endmodule
