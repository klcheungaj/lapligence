// IEEE 1800-2009 6.16: string values use owned byte storage across
// subroutine formals, aliases, returns, and delayed task copy-out.
module tb;
    string base;
    string result;
    string const_result;

    function automatic string append_mark(input string value);
        append_mark = {value, "!"};
    endfunction

    function automatic string mutate_ref(ref string value);
        value.putc(1, 8'h59);
        mutate_ref = value;
    endfunction

    function automatic string read_const(const ref string value);
        read_const = value;
    endfunction

    function string static_copy(input string value);
        string saved;
        saved = value;
        static_copy = saved;
    endfunction

    task automatic rewrite(output string value);
        #1;
        value = "out";
    endtask

    task automatic append_later(inout string value, input string suffix);
        #1;
        value = {value, suffix};
    endtask

    initial begin
        base = "abcd";
        result = append_mark(base);
        if (result != "abcd!" || base != "abcd") begin
            $display("FAIL string_subroutine_forms input_return");
            $finish;
        end

        result = mutate_ref(base);
        const_result = read_const(base);
        if (result != "aYcd" || base != "aYcd" || const_result != "aYcd") begin
            $display("FAIL string_subroutine_forms ref_const_ref");
            $finish;
        end

        if (static_copy("one") != "one" || static_copy("two") != "two") begin
            $display("FAIL string_subroutine_forms static_return");
            $finish;
        end

        rewrite(result);
        append_later(result, "!");
        if (result != "out!") begin
            $display("FAIL string_subroutine_forms delayed_copyout");
            $finish;
        end

        $display("PASS string_subroutine_forms");
        $finish;
    end
endmodule
