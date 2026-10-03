// IEEE 1800-2009 §7.3.2 and §11.9: an unpacked tagged union with fixed
// integral payloads uses the same finite tag-plus-payload storage as the
// packed form; its member accesses are tag-checked.
typedef union tagged {
    void empty;
    logic [7:0] narrow;
} tagged_t;

module tb;
    tagged_t value;

    initial begin
        value = tagged narrow(8'h5a);
        if (value.narrow !== 8'h5a)
            $fatal(1, "unpacked tagged member read");
        value = tagged empty;
        if (value matches tagged empty) ;
        else $fatal(1, "unpacked tagged void member");
        $display("PASS syn_021_tagged_unpacked_values");
        $finish;
    end
endmodule
