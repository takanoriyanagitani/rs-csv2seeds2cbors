(module

  (import "host" "print_i32" (func $print_i32 (param i32)))
  (import "host" "print_i64" (func $print_i64 (param i64)))

  (import "host" "a2unsigned"
    (func $a2unsigned (param $ofst i32) (param $siz i32) (result i64)))

  (import "host" "request_seeds" (func $request_seeds (param i32)(result i32)))
  (import "host" "cbor_docs_wrote"
    (func $cbor_docs_wrote (param $ofst i32) (param $siz i32) (result i32)))

  (memory (export "memory") 2)

  (global $OUT_PTR i32 (i32.const 0x0001_0000))

  (func $bswap16 (param $i i32) (result i32)
    local.get $i                  ;; 0x0000_ZZWW
    i32.const 0x0000_00ff i32.and ;; 0x0000_00WW
    i32.const 8 i32.shl           ;; 0x0000_WW00

    local.get $i                  ;; 0x0000_ZZWW
    i32.const 8 i32.shr_u         ;; 0x0000_00ZZ

    i32.or
  )

  (func $bswap64 (param $i i64) (result i64)
    (local $tmp i64)
                                      ;; goal: 0xWWZZ_YYXX_SSRR_QQPP
    local.get $i                            ;; 0xPPQQ_RRSS_XXYY_ZZWW
    i64.const 0x00ff_00ff_00ff_00ff i64.and ;; 0x00QQ_00SS_00YY_00WW
    i64.const 8 i64.shl                     ;; 0xQQ00_SS00_YY00_WW00

    local.get $i                            ;; 0xPPQQ_RRSS_XXYY_ZZWW
    i64.const 0xff00_ff00_ff00_ff00 i64.and ;; 0xPP00_RR00_XX00_ZZ00
    i64.const 8 i64.shr_u                   ;; 0x00PP_00RR_00XX_00ZZ

    i64.or                                  ;; 0xQQPP_SSRR_YYXX_WWZZ

    local.tee $tmp                          ;; 0xQQPP_SSRR_YYXX_WWZZ
    i64.const 0x0000_ffff_0000_ffff i64.and ;; 0x0000_SSRR_0000_WWZZ
    i64.const 16 i64.shl                    ;; 0xSSRR_0000_WWZZ_0000

    local.get $tmp                          ;; 0xQQPP_SSRR_YYXX_WWZZ
    i64.const 0xffff_0000_ffff_0000 i64.and ;; 0xQQPP_0000_YYXX_0000
    i64.const 16 i64.shr_u                  ;; 0x0000_QQPP_0000_YYXX

    i64.or                                  ;; 0xSSRR_QQPP_WWZZ_YYXX
    i64.const 32 i64.shl                    ;; 0xWWZZ_YYXX_SSRR_QQPP
  )

  (func $io_main (export "io_main")
    (local $seeds_len i32)

    (local $ptr i32)
    (local $slen i32)

    (local $id     i64)
    (local $sub_id i64)
    (local $date8  i64)

    (local $cbor_val_i i64)
    (local $cbor_write_ret i32)

    loop
      i32.const 0 call $request_seeds local.tee $seeds_len
      ;; return when the len < 0(error or EOF)
      i32.const 0 i32.lt_s if
        return
      end

      ;;;;;;;;;;;;;
      ;; read id ;;
      ;;;;;;;;;;;;;
      i32.const 0 local.set $ptr
      local.get $ptr i32.load8_u offset=0 i32.const 0x79 i32.ne if
        return ;; expected string up to 65536 bytes
      end

      local.get $ptr i32.load16_u offset=1 call $bswap16 local.set $slen

      local.get $ptr i32.const 3 i32.add local.get $slen call $a2unsigned
      local.tee $id
      i64.const 0 i64.lt_s if
        return ;; invalid csv value
      end

      ;;;;;;;;;;;;;;;;;
      ;; read sub id ;;
      ;;;;;;;;;;;;;;;;;
      local.get $ptr i32.const 3 i32.add local.get $slen i32.add
      local.set $ptr
      local.get $ptr i32.load8_u offset=0 i32.const 0x79 i32.ne if
        return ;; expected string up to 65536 bytes
      end

      local.get $ptr i32.load16_u offset=1 call $bswap16 local.set $slen

      local.get $ptr i32.const 3 i32.add local.get $slen call $a2unsigned
      local.tee $sub_id
      i64.const 0 i64.lt_s if
        return ;; invalid csv value
      end

      ;;;;;;;;;;;;;;;;
      ;; read date8 ;;
      ;;;;;;;;;;;;;;;;
      local.get $ptr i32.const 3 i32.add local.get $slen i32.add
      local.set $ptr
      local.get $ptr i32.load8_u offset=0 i32.const 0x79 i32.ne if
        return ;; expected string up to 65536 bytes
      end

      local.get $ptr i32.load16_u offset=1 call $bswap16 local.set $slen

      local.get $ptr i32.const 3 i32.add local.get $slen call $a2unsigned
      local.tee $date8
      i64.const 0 i64.lt_s if
        return ;; invalid csv value
      end

      ;;local.get $id     call $print_i64
      ;;local.get $sub_id call $print_i64
      ;;local.get $date8  call $print_i64

      local.get $id
      local.get $sub_id
      local.get $date8
      i64.or
      i64.or
      local.set $cbor_val_i

      global.get $OUT_PTR i32.const 0x1b i32.store8
      global.get $OUT_PTR local.get $cbor_val_i call $bswap64 i64.store offset=1

      global.get $OUT_PTR i32.const 9 call $cbor_docs_wrote
      local.set $cbor_write_ret

      br 0
    end
  )

)
