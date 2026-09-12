// Descriptor-backed shortreal containers apply destination rounding.
module tb;
    real real_source[];
    shortreal short_destination[];
    real real_roundtrip[];

    initial begin
        real_source = new[1];
        real_source[0] = $bitstoreal(64'h3ff0000002000000);
        short_destination = new[1];
        short_destination[0] = real_source[0];
        real_roundtrip = new[1];
        real_roundtrip[0] = short_destination[0];
        if ($shortrealtobits(short_destination[0]) !== 32'h3f800000 ||
            $realtobits(real_roundtrip[0]) !== 64'h3ff0000000000000) begin
            $display("FAIL container_copy_conversion shortreal_rounding");
            $finish;
        end

        $display("PASS container_copy_conversion");
        $finish;
    end
endmodule
