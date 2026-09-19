// IEEE 1800-2009 13.5.5: a ref formal aliases the actual storage cell, so
// writes are immediately visible; legal actuals are integral variable
// storage (including fixed unpacked-array elements).
module tb;
    task automatic set_ref(ref int x);
        x = 21;
        x = x * 2;
    endtask

    task automatic set_elem(ref int x);
        x = x + 1;
    endtask

    int v;
    int m [0:3];
    initial begin
        v = 0;
        set_ref(v);
        $display("v=%0d", v);
        m[2] = 10;
        set_elem(m[2]);
        $display("m2=%0d", m[2]);
        $finish(0);
    end
endmodule
