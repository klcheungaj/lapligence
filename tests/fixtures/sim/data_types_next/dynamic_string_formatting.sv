// IEEE 1800-2009 5.9.1, 6.8, 6.16, and 21.2: string literals preserve
// escaped character bytes, static declaration initializers run before initial
// procedures, and $write differs from $display only by its trailing newline.
module tb;
    logic [31:0] packed_seed = "seed";
    string seeded = string'(packed_seed);
    string shadowed = "GLOBAL";
    string escaped;
    string formal_result;

    function automatic string convert_shadow(input logic [47:0] shadowed);
        convert_shadow = string'(shadowed);
    endfunction

    initial begin
        escaped = "N:line1\nline2\t\101\x42\xff";
        formal_result = convert_shadow("formal");

        if (seeded != "seed" || shadowed != "GLOBAL" ||
            formal_result != "formal" || escaped.len() !== 17 ||
            escaped.getc(7) !== 8'h0a || escaped.getc(13) !== 8'h09 ||
            escaped.getc(14) !== 8'h41 || escaped.getc(15) !== 8'h42 ||
            escaped.getc(16) !== 8'hff) begin
            $display("FAIL dynamic_string_formatting values");
            $finish;
        end

        $write("WRITE<%s>|seed=%s|formal=%s", escaped, seeded, formal_result);
        $display("|global=%s", shadowed);
        $display("PASS dynamic_string_formatting");
        $finish;
    end
endmodule
