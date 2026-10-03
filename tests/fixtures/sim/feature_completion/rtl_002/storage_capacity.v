// IEEE 1364-2001 §3.10 / IEEE 1800-2009 §7.4.2: selected storage in both editions.
module tb;
    reg a [0:65534];
    reg b [65535:0];
    reg c [-3:65533];
    reg matrix [4095:0][-2048:2047];
    integer invalid;
    initial begin
        a[65534] = 1;
        b[65535] = 1;
        c[65533] = 1;
        matrix[4095][-2048] = 1;
        matrix[0][2047] = 0;
        invalid = -1;
        matrix[invalid][0] = 0;
        if (a[65534] !== 1'b1 || b[65535] !== 1'b1 || c[65533] !== 1'b1 ||
            matrix[4095][-2048] !== 1'b1 || matrix[0][2047] !== 1'b0 || matrix[1][0] !== 1'bx)
            $display("FAIL rtl002 storage");
        else $display("PASS rtl002 storage");
        $finish(0);
    end
endmodule
