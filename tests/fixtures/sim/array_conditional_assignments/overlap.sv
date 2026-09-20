module tb;
    typedef logic [7:0] array_t [0:3];
    array_t a, b;
    logic selector;
    initial begin
        a = '{8'h11, 8'h22, 8'h33, 8'h44};
        b = '{8'h55, 8'h66, 8'h77, 8'h88};
        selector = 1;
        a[1:2] = selector ? a[0:1] : b[0:1];
        if (a[0] !== 8'h11 || a[1] !== 8'h11 || a[2] !== 8'h22 || a[3] !== 8'h44)
            $fatal(1, "overlapping slice must use old source elements");
        a = selector ? array_t'{a[3], a[0], a[1], a[2]} : b;
        if (a[0] !== 8'h44 || a[1] !== 8'h11 || a[2] !== 8'h11 || a[3] !== 8'h22)
            $fatal(1, "overlapping whole array must be captured before writes");
        selector = 0;
        a[0:1] = selector ? b[2:3] : a[2:3];
        if (a[0] !== 8'h11 || a[1] !== 8'h22) $fatal(1, "false slice source");
        $display("overlap=%h,%h,%h,%h", a[0], a[1], a[2], a[3]);
        $finish(0);
    end
endmodule
