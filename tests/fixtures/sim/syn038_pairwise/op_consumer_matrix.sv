// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv
typedef logic [7:0] byte_t;
typedef bit [7:0] two_byte_t;

module consumer(input logic [7:0] value, output logic [7:0] observed);
    assign observed = value;
endmodule

module tb;
    parameter bit CONST_SELECT = 1'b1;
    parameter byte_t CONST_A = 8'h12;
    parameter byte_t CONST_B = 8'h34;
    typedef logic [(CONST_SELECT ? 5 : 7)-1:0] conditional_width_t;
    typedef logic [((CONST_A == CONST_B) ? 5 : 7)-1:0] equality_width_t;
    typedef logic [(int'(CONST_A[3:0]) - 1):0] cast_width_t;

    logic select;
    byte_t a, b;
    byte_t conditional_decl = CONST_SELECT ? CONST_A : CONST_B;
    int conditional_call, equality_call, cast_call, pattern_call;
    int event_conditional, event_cast, event_pattern;
    logic armed;
    byte_t observed;
    consumer c0(.value(two_byte_t'(a)), .observed(observed));

    function automatic int consume(input byte_t value);
        return int'(value);
    endfunction
    function automatic int consume_bit(input bit value);
        return int'(value);
    endfunction
    function automatic bit equal_return(input byte_t x, input byte_t y);
        return x == y;
    endfunction

    always @(select ? a[0] : b[0]) if (armed) event_conditional++;
    always @(two_byte_t'(a)) if (armed) event_cast++;
    always @(byte_t'{a[7],a[6],a[5],a[4],a[3],a[2],a[1],a[0]}) if (armed) event_pattern++;

    initial begin
        select = 1'b1; a = 8'h12; b = 8'h34; armed = 1'b0;
        event_conditional = 0; event_cast = 0; event_pattern = 0;
        #1;
        armed = 1'b1;
        conditional_call = consume(select ? a : b);
        equality_call = consume_bit(a == b);
        cast_call = consume(two_byte_t'(a));
        pattern_call = consume('{a[7],a[6],a[5],a[4],a[3],a[2],a[1],a[0]});
        if (conditional_decl != 8'h12) $fatal(1,"decl");
        if (observed != 8'h12) $fatal(1,"port");
        if (conditional_call != 18 || equality_call != 0 || cast_call != 18 || pattern_call != 18) $fatal(1,"calls");
        if (equal_return(a,b) != 0) $fatal(1,"return");
        a = 8'h13;
        #1;
        if (event_conditional < 1 || event_cast < 1 || event_pattern < 1) $fatal(1,"events");
        if ($bits(conditional_width_t) != 5 || $bits(equality_width_t) != 7 || $bits(cast_width_t) != 2) $fatal(1,"widths");
        $display("calls=%0d,%0d,%0d,%0d events=%0d,%0d,%0d widths=%0d,%0d,%0d",conditional_call,equality_call,cast_call,pattern_call,event_conditional,event_cast,event_pattern,$bits(conditional_width_t),$bits(equality_width_t),$bits(cast_width_t));
        $finish(0);
    end
endmodule
