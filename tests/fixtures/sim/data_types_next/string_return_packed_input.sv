// IEEE 1800-2009 6.16 and 13.4.1: explicit return exits a nonvoid function
// with its expression, and the implicit function-name variable can be assigned,
// read, and copied. Packed inputs are converted explicitly to dynamic strings.
module tb;
    string early_result;
    string packed_result;
    string fallback_result;
    string copied_result;

    function automatic string choose_text(
        input logic [7:0] selector,
        input logic [31:0] packed_text
    );
        if (selector == 8'd0)
            return "early";
        if (selector == 8'd1)
            return string'(packed_text);
        return "fallback";
    endfunction

    function automatic string duplicate_text(input logic [15:0] packed_text);
        duplicate_text = string'(packed_text);
        duplicate_text = {duplicate_text, duplicate_text};
    endfunction

    initial begin
        early_result = choose_text(8'd0, "late");
        packed_result = choose_text(8'd1, "word");
        fallback_result = choose_text(8'd2, "late");
        copied_result = duplicate_text("Hi");

        if (early_result != "early" || packed_result != "word" ||
            fallback_result != "fallback" || copied_result != "HiHi") begin
            $display("FAIL string_return_packed_input");
            $finish;
        end

        $display("PASS string_return_packed_input");
        $finish;
    end
endmodule
