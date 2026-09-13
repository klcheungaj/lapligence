interface signals;
    wire w; tri t; uwire u;
endinterface
module leaf(output wire [64:0] w, output uwire [64:0] u);
endmodule
module tb;
    wire [64:0] w;
    uwire [64:0] u;
    wire [64:0] array_w[0:1];
    uwire [64:0] array_u[0:1];
    signals s();
    leaf child(w,u);
    initial begin
        #1;
        if (w !== 'z || u !== 'z || child.w !== 'z || child.u !== 'z ||
            s.w !== 1'bz || s.t !== 1'bz || s.u !== 1'bz ||
            array_w[0] !== 'z || array_w[1] !== 'z ||
            array_u[0] !== 'z || array_u[1] !== 'z) $display("FAIL net defaults");
        $display("PASS net defaults"); $finish(0);
    end
endmodule
