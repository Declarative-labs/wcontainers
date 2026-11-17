// Stands in for the C library's assert.h in the converters' builds: a failed assertion calls
// `wcs_assert_failed` (cpp/assertion.cpp), which throws where exceptions are on, so malformed bytecode
// becomes a refused conversion instead of ending the process. No include guard, as the standard's.

#undef assert
#ifdef NDEBUG
#define assert(e) ((void)0)
#else
#define assert(e) ((e) ? (void)0 : wcs_assert_failed(#e, __FILE__, __LINE__))
#endif

#ifndef WCS_ASSERT_FAILED
#define WCS_ASSERT_FAILED
extern "C++" [[noreturn]] void wcs_assert_failed(const char* expression, const char* file, int line);
#endif
