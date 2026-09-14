// R03 scalar subcases: mismatch differs from emptiness and never consumes.
module tb;
    mailbox box = new;
    int value = 99, status;
    int unsigned unsigned_value = 17;
    bit signed [31:0] equivalent;
    integer four_state = -1;
    byte narrow = -2;
    string text = "unchanged";
    shortreal small = 3.5;
    real wide = 1.25, copy;
    initial begin
        status = box.try_get(value);
        $display("empty=%0d value=%0d", status, value);
        box.put(42);
        status = box.try_get(text);
        $display("text=%0d value=%s n=%0d", status, text, box.num());
        status = box.try_peek(narrow);
        $display("width=%0d value=%0d n=%0d", status, narrow, box.num());
        status = box.try_get(four_state);
        $display("state=%0d value=%0d n=%0d", status, four_state, box.num());
        status = box.try_get(unsigned_value);
        $display("sign=%0d value=%0d n=%0d", status, unsigned_value, box.num());
        status = box.try_get(equivalent);
        $display("get=%0d value=%0d n=%0d", status, equivalent, box.num());
        box.put(wide);
        status = box.try_peek(small);
        $display("real_kind=%0d n=%0d", status, box.num());
        box.get(copy);
        $display("real=%0.2f n=%0d", copy, box.num());
        $finish(0);
    end
endmodule
