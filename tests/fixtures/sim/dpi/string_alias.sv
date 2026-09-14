// R18: all foreign string results must be copied before destructive copy-out.
module tb;
    import "DPI-C" priority_echo = function string echo(inout string s);
    import "DPI-C" priority_swap = function string swap(inout string a, inout string b);
    import "DPI-C" priority_swap_void = function void swap_void(inout string a, inout string b);
    import "DPI-C" priority_share = function string share(
        inout string source, output string first, output string second);

    string a, b, result, first, second;
    initial begin
        a = "one";
        result = echo(a);
        $display("echo=%s/%s", result, a);
        a = "left";
        b = "right";
        result = swap(a, b);
        $display("swap=%s/%s/%s", result, a, b);
        swap_void(a, b);
        $display("void=%s/%s", a, b);
        a = "source";
        result = share(a, first, second);
        first.putc(0, 8'd83);
        $display("share=%s/%s/%s/%s", result, a, first, second);
        $finish(0);
    end
endmodule
