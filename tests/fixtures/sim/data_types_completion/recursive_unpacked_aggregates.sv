// IEEE 1800-2009 7.2 and 7.3: recursive unpacked aggregate values, fixed
// unpacked arrays, and unequal-width untagged union member storage.
module tb;
    typedef struct {
        logic [7:0] bytes [0:1];
        real number;
        string text;
    } inner_t;

    typedef struct {
        inner_t inner;
        logic [3:0] tag;
    } outer_t;

    typedef union {
        logic [3:0] narrow;
        logic [7:0] wide;
    } overlay_t;

    outer_t original;
    outer_t copy;
    outer_t defaults;
    overlay_t overlay;

    initial begin
        original = '{inner: '{bytes: '{8'h11, 8'h22}, number: 1.25, text: "alpha"}, tag: 4'h3};
        copy = original;
        if (original != copy) begin
            $display("FAIL recursive_unpacked_aggregates");
            $finish;
        end
        copy.inner.bytes[1] = 8'haa;
        copy.inner.number = 2.5;
        copy.inner.text = "beta";
        copy.tag = 4'hc;

        overlay.narrow = 4'h5;
        if (original.inner.bytes[0] !== 8'h11
            || original.inner.bytes[1] !== 8'h22
            || original.inner.number != 1.25
            || original.inner.text != "alpha"
            || original.tag !== 4'h3
            || copy.inner.bytes[0] !== 8'h11
            || copy.inner.bytes[1] !== 8'haa
            || copy.inner.number != 2.5
            || copy.inner.text != "beta"
            || copy.tag !== 4'hc
            || original == copy
            || defaults.inner.bytes[0] !== 8'hxx
            || defaults.inner.number != 0.0
            || defaults.inner.text != ""
            || overlay.wide !== 8'hx5
            || overlay.narrow !== 4'h5) begin
            $display("FAIL recursive_unpacked_aggregates");
            $finish;
        end
        $display("PASS recursive_unpacked_aggregates");
        $finish;
    end
endmodule
