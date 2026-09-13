module real_child(
    input real in_value,
    input shortreal in_short,
    output real out_value
);
    always_comb out_value = in_value + in_short;
endmodule

module tb;
    real value;
    shortreal short_value;
    real doubled;
    real port_value;
    integer wait_hits = 0;
    integer event_hits = 0;
    integer expression_hits = 0;
    integer comb_hits = 0;
    integer signed_zero = 0;
    integer nan_seen = 0;
    integer case_hits = 0;
    real case_value;

    real_child child(
        .in_value(value),
        .in_short(short_value),
        .out_value(port_value)
    );

    always_comb begin
        doubled = value * 2.0;
        comb_hits = comb_hits + 1;
    end

    initial begin
        @(value);
        event_hits = event_hits + 1;
        @(value);
        event_hits = event_hits + 1;
        @(value);
        event_hits = event_hits + 1;
    end

    initial begin
        wait (value);
        wait_hits = wait_hits + 1;
    end

    initial begin
        @(value + 1.0);
        expression_hits = expression_hits + 1;
        @(value + 1.0);
        expression_hits = expression_hits + 1;
        @(value + 1.0);
        expression_hits = expression_hits + 1;
    end

    initial begin
        value = 0.0;
        short_value = 0.0;
        #1 value = 1.5;
        #1 value = 1.5;
        #1 short_value = 16777217.0;
        #1 $display("port=%.1f short=%.0f", port_value, short_value);
        #1 value = $bitstoreal(64'h8000000000000000);
        signed_zero = ($realtobits(value) == 64'h8000000000000000);
        #1 value = $bitstoreal(64'h7ff8000000000000);
        nan_seen = (value != value);
        #1 value = $bitstoreal(64'h7ff8000000000000);
        case_value = 2.5;
        case (case_value)
            1.5: case_hits = 1;
            2.5: case_hits = 2;
            default: case_hits = 3;
        endcase
        #1 $display("wait=%0d events=%0d expr=%0d comb=%0d signed_zero=%0d nan=%0d doubled_nan=%0d",
                   wait_hits, event_hits, expression_hits, comb_hits, signed_zero, nan_seen,
                   doubled != doubled);
        $display("case=%0d", case_hits);
        $finish(0);
    end
endmodule
