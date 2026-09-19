// IEEE 1800-2009 10.9.1-10.9.2: keyed nested struct/array patterns use
// member names over defaults, and defaults recurse into nested arrays and
// structures.  The same recursive leaf walker serves whole-aggregate and
// nested sub-aggregate patterns, and nested sub-aggregate copies are deep.
module tb;
    typedef logic [7:0] lane_t;
    typedef struct {
        lane_t value;
        lane_t bytes [0:1];
    } inner_t;
    typedef struct {
        inner_t items [0:1];
        lane_t tail;
    } outer_t;

    outer_t got;
    outer_t expected;

    initial begin
        got.items[0] = '{value: 8'h5a, bytes: '{default: 8'h3c, 1: 8'h4d}};
        got.items[1] = '{default: 8'h11, value: 8'h22};
        got.tail = 8'h6e;

        expected = '{items: '{
            '{value: 8'h5a, bytes: '{8'h3c, 8'h4d}},
            '{value: 8'h22, bytes: '{8'h11, 8'h11}}
        }, tail: 8'h6e};
        if (got != expected) begin
            $display("FAIL nested_default");
            $finish;
        end

        // A nested sub-aggregate copy is deep: mutating the destination
        // element after the copy does not disturb the source element.
        got.items[1] = got.items[0];
        got.items[0] = '{value: 8'h5a, bytes: '{8'h99, 8'h4d}};
        expected = '{items: '{
            '{value: 8'h5a, bytes: '{8'h99, 8'h4d}},
            '{value: 8'h5a, bytes: '{8'h3c, 8'h4d}}
        }, tail: 8'h6e};
        if (got != expected) begin
            $display("FAIL nested_copy_independence");
            $finish;
        end
        $display("PASS pattern_nested_default");
        $finish;
    end
endmodule
