// IEEE 1800-2009 §7.3.2 and §11.9: packed tagged unions retain a finite
// tag alongside the maximum member payload, and member reads check that tag.
typedef union tagged packed {
    void empty;
    logic [7:0] narrow;
    logic [63:0] wide_value;
} tagged_t;

typedef union tagged packed {
    void none;
    tagged_t inner;
    logic [63:0] wide_value;
} nested_t;

module echo(input tagged_t in_value, output tagged_t out_value);
    always_comb out_value = in_value;
endmodule

module tb;
    tagged_t value;
    tagged_t values [0:1];
    nested_t nested;
    tagged_t echoed;
    tagged_t copied;
    integer function_calls;

    function automatic tagged_t make_small(input logic [7:0] arg);
        function_calls = function_calls + 1;
        make_small = tagged narrow(arg);
    endfunction

    function automatic tagged_t identity(input tagged_t arg);
        identity = arg;
    endfunction

    function automatic logic [7:0] read_narrow(input tagged_t arg);
        read_narrow = arg.narrow;
    endfunction

    echo u_echo(.in_value(value), .out_value(echoed));

    initial begin
        function_calls = 0;
        value = tagged empty;
        if ($bits(tagged_t) !== 66)
            $fatal(1, "void constructor or layout");

        value = make_small(8'h5a);
        value = identity(value);
        values[0] = value;
        values[1] = tagged wide_value(64'hd2);
        #1;
        if (function_calls !== 1 || value.narrow !== 8'h5a ||
            values[0].narrow !== 8'h5a ||
            values[1].wide_value !== 64'hd2 || echoed.narrow !== 8'h5a ||
            read_narrow(value) !== 8'h5a)
            $fatal(1, "constructor, function, array, or port");

        copied = value;
        if (copied !== value)
            $fatal(1, "tagged copy or equality");
        copied = tagged_t'(value);
        if (copied.narrow !== 8'h5a)
            $fatal(1, "tagged cast");

        if (value.wide_value !== 64'hx)
            $fatal(1, "inactive member read");

        nested = tagged inner(value);
        if (nested.inner.narrow !== 8'h5a ||
            nested.inner.wide_value !== 64'hx)
            $fatal(1, "nested member read");

        nested = tagged wide_value(64'hab5);
        if (nested.wide_value !== 64'hab5 || nested.inner.narrow !== 8'hx)
            $fatal(1, "outer inactive member read");

        $display("PASS syn_021_tagged_values");
        $finish;
    end
endmodule
