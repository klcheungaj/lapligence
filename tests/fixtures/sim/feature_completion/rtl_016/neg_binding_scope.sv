// IEEE 1800-2009 12.6: a pattern identifier is scoped to the matching
// statement's true arm; using it after the statement is illegal.
typedef union tagged { void Empty; int Count; } item_t;

module tb;
    item_t value;
    int copy;
    initial begin
        value = tagged Count (3);
        if (value matches tagged Count .n) copy = n;
        copy = n;
    end
endmodule
