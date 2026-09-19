// IEEE 1800-2009 10.9.1-10.9.2: an explicit positional or named expression is
// evaluated exactly once.  The number of evaluations of a type-key/default/
// replication expression with side effects is explicitly undefined, so the
// observation is only that it is evaluated at least once and yields the same
// value for every leaf it covers.
module tb;
    typedef logic [7:0] lane_t;
    typedef struct {
        lane_t a;
        lane_t b;
        lane_t c;
    } triple_t;

    integer explicit_calls;
    integer default_calls;

    function automatic lane_t next_explicit();
        explicit_calls = explicit_calls + 1;
        next_explicit = 8'h5a;
    endfunction

    function automatic lane_t next_default();
        default_calls = default_calls + 1;
        next_default = 8'h7e;
    endfunction

    triple_t keyed;
    triple_t positional;

    initial begin
        explicit_calls = 0;
        default_calls = 0;
        keyed = '{a: next_explicit(), default: next_default()};
        if (explicit_calls != 1) begin
            $display("FAIL keyed_explicit_calls=%0d", explicit_calls);
            $finish;
        end
        if (default_calls < 1) begin
            $display("FAIL keyed_default_never_evaluated");
            $finish;
        end
        if (keyed.a !== 8'h5a || keyed.b !== 8'h7e || keyed.c !== 8'h7e) begin
            $display("FAIL keyed_values");
            $finish;
        end

        explicit_calls = 0;
        default_calls = 0;
        positional = '{next_explicit(), next_default(), next_explicit()};
        if (explicit_calls != 2) begin
            $display("FAIL positional_explicit_calls=%0d", explicit_calls);
            $finish;
        end
        if (positional.a !== 8'h5a || positional.b !== 8'h7e
                || positional.c !== 8'h5a) begin
            $display("FAIL positional_values");
            $finish;
        end
        $display("PASS pattern_side_effect_count");
        $finish;
    end
endmodule
