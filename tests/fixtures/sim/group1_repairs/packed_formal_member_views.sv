module tb;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } bytes_t;
    typedef union packed { logic [15:0] raw; bytes_t parts; bit [15:0] clean; } word_t;
    typedef struct packed { logic [7:0] tag; logic [0:7] lane; } ascending_t;
    word_t original, changed;
    ascending_t a, b;
    function automatic word_t update(input word_t copy);
        copy.clean[7:0] += 8'd1;
        if (copy.raw[15:8] !== 8'hxx) $fatal(1, "neighbor union bits were coerced");
        return copy;
    endfunction
    function automatic ascending_t select_lane(input ascending_t copy);
        copy.lane[2 +: 3] = 3'b111;
        if (copy.lane[6 +: 4] !== 4'b00xx) $fatal(1, "selected member bounds");
        return copy;
    endfunction
    typedef struct packed {
        logic [7:0] tag;
        logic signed [7:0] value;
    } signed_member_t;
    signed_member_t sign_source;
    function automatic signed_member_t halve(input signed_member_t value);
        value.value >>>= 1;
        return value;
    endfunction
    initial begin
        sign_source = 16'ha580;
        if (halve(sign_source) !== 16'ha5c0 || sign_source !== 16'ha580)
            $fatal(1, "whole member sign and private owner");
        original.raw = 16'hxxxx;
        changed = update(original);
        if (changed.raw !== 16'hxx01 || original.raw !== 16'hxxxx)
            $fatal(1, "two-state union member read-modify-write");
        a = 16'ha500;
        b = select_lane(a);
        if (b !== 16'ha538 || a !== 16'ha500) $fatal(1, "ascending packed member");
        $display("packed member views passed");
        $finish(0);
    end
endmodule
