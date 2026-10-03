// IEEE 1800-2009 §§11.4.14, 21.4: descriptor streams and sparse memory views.
module tb;
    typedef logic [7:0] memory_t [0:16777215];
    memory_t source, target;
    integer descriptor, count;
    initial begin
        source[0] = 8'h81;
        source[16777215] = 8'h12;
        target = {>>{source}};
        if (target[0] !== 8'h81 || target[16777215] !== 8'h12 || target[4] !== 8'bx) $fatal;
        target = {<<8{source}};
        if (target[0] !== 8'h12 || target[16777215] !== 8'h81 || target[4] !== 8'bx) $fatal;
        target = {<<{source}};
        if (target[0] !== 8'h48 || target[16777215] !== 8'h81) $fatal;
        source = {<<8{source}};
        if (source[0] !== 8'h12 || source[16777215] !== 8'h81) $fatal;
        $writememh("selected.hex", source, 0, 1);
        target[0] = 0;
        $readmemh("selected.hex", target, 0, 1);
        if (target[0] !== 8'h12 || target[1] !== 8'bx || target[2] !== 8'bx) $fatal;
        descriptor = $fopen("byte.bin", "wb");
        $fwrite(descriptor, "%c", 8'h5a);
        $fclose(descriptor);
        descriptor = $fopen("byte.bin", "rb");
        count = $fread(target, descriptor, 16777215, 1);
        $fclose(descriptor);
        if (count !== 1 || target[16777215] !== 8'h5a) $fatal;
        $display("PASS rtl002 stream services");
        $finish(0);
    end
endmodule
